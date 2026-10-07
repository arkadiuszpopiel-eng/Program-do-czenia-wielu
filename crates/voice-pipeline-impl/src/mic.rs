//! Tor mikrofonu: wejścia UI, aktywacja (`voice-wake` + dzierżawa mikrofonu), otwarcie strumienia
//! tylko na czas słuchania, DSP z AEC, reguła echa, VAD, pre-roll i ramki do STT.

use voice_audio_contract::gain::rms_db;
use voice_audio_contract::{Frame, StreamConfig};
use voice_dialog_contract::{ActivationSource, DialogEvent, ProactiveLabel};
use voice_dsp_contract::Processed;
use voice_pipeline_contract::{PipelineInput, SwitchSource};
use voice_turn_contract::TurnEvent;
use voice_vad_contract::VadEvent;
use voice_wake_contract::{WakeEvent, WakeInput, WakeSource};

use crate::pipeline::Pipeline;

impl Pipeline {
    /// Wejścia UI z kolejki (w kolejności).
    pub(crate) fn process_inputs(&mut self) {
        while let Some(input) = self.st.inputs.pop_front() {
            match input {
                PipelineInput::Ptt { pressed } => self.wake_input(WakeInput::UiPtt { pressed }),
                PipelineInput::Toggle => self.wake_input(WakeInput::UiToggle),
                PipelineInput::SetMuted { muted } => {
                    self.wake_input(WakeInput::SetMuted { muted });
                    self.wake_words_flags(Some(muted), None);
                }
                PipelineInput::SetDoNotDisturb { on } => {
                    self.wake_input(WakeInput::SetDnd { on });
                    self.wake_words_flags(None, Some(on));
                    self.dialog_event(DialogEvent::SetDoNotDisturb { enabled: on });
                }
                PipelineInput::Typed { text } => self.dialog_event(DialogEvent::UserTyped { text }),
                PipelineInput::StopSpeech => self.dialog_event(DialogEvent::StopSpeech),
                PipelineInput::Deactivate => self.dialog_event(DialogEvent::Deactivate),
                PipelineInput::SwitchPersona { persona } => {
                    self.switch_persona(persona, SwitchSource::Ui);
                }
                PipelineInput::Proactive {
                    persona,
                    text,
                    reason,
                } => self.dialog_event(DialogEvent::ProactiveRequest {
                    persona: persona.clone(),
                    text,
                    label: ProactiveLabel {
                        who: persona,
                        reason,
                    },
                }),
            }
        }
    }

    /// Wejście automatu aktywacji + obsługa zdarzeń.
    pub(crate) fn wake_input(&mut self, input: WakeInput) {
        let events = self.wake.handle(input);
        self.on_wake_events(events);
    }

    /// Zdarzenia aktywacji: dzierżawa mikrofonu, otwarcie/zamknięcie strumienia, magistrala.
    pub(crate) fn on_wake_events(&mut self, events: Vec<WakeEvent>) {
        if events.is_empty() {
            return;
        }
        if let Err(e) = self.mic_lease.apply_now(&events) {
            self.degraded("mic", format!("mikrofon zajęty: {e}"));
            for ev in events
                .iter()
                .filter(|e| !matches!(e, WakeEvent::ListenStart { .. }))
            {
                self.outbox.push(ev.to_bus_event());
            }
            self.force_stop_listening();
            return;
        }
        for ev in events {
            self.outbox.push(ev.to_bus_event());
            match ev {
                WakeEvent::ListenStart { source, .. } => self.open_mic(source),
                WakeEvent::ListenStop { .. } => self.close_mic(),
                _ => {}
            }
        }
        if self.wake_words.is_some() {
            self.sync_wake_suspension();
        }
    }

    /// Zamyka słuchanie niezależnie od źródła (wyciszenie i odciszenie automatu aktywacji).
    pub(crate) fn force_stop_listening(&mut self) {
        let mut events = self.wake.handle(WakeInput::SetMuted { muted: true });
        events.extend(self.wake.handle(WakeInput::SetMuted { muted: false }));
        self.on_wake_events(events);
    }

    fn open_mic(&mut self, source: WakeSource) {
        if self.mic.is_none() {
            match self.audio.open_input(None, &StreamConfig::input_default()) {
                Ok(stream) => {
                    self.mic = Some(stream);
                    self.dsp.reset();
                    self.vad.reset();
                    self.st.preroll.clear();
                }
                Err(e) => {
                    self.degraded("audio", format!("mikrofon nie otworzył się: {e}"));
                    self.force_stop_listening();
                    return;
                }
            }
        }
        self.st.addressed = true;
        self.st.last_activity_ms = self.st.now_ms;
        let source = match source {
            WakeSource::WakeWord => ActivationSource::WakeWord,
            WakeSource::Ptt | WakeSource::Toggle | WakeSource::Name | WakeSource::Ui => {
                ActivationSource::PushToTalk
            }
        };
        self.dialog_event(DialogEvent::Activate { source });
    }

    /// Zamyka strumień mikrofonu (przy uzbrojonych słowach wywoławczych zostaje otwarty dla
    /// nasłuchu); trwająca wypowiedź kończy się od razu (puszczenie PTT).
    fn close_mic(&mut self) {
        if self.wake_words.is_none() {
            self.mic = None;
        }
        self.vad.reset();
        let speaking = self
            .st
            .user
            .as_ref()
            .is_some_and(|u| !u.closing && u.vad_active);
        if speaking {
            let now = self.st.now_ms;
            self.on_vad(VadEvent::SpeechEnd {
                ts: voice_audio_contract::MediaTime::from_ms(now),
                duration: std::time::Duration::ZERO,
            });
        }
        if self.st.user.as_ref().is_some_and(|u| !u.closing) {
            self.close_user();
        }
    }

    /// Ramki mikrofonu z kolejki przechwytywania → DSP → reguła echa → VAD → STT.
    pub(crate) fn process_mic(&mut self) {
        let Some(mic) = self.mic.as_mut() else {
            return;
        };
        let mut frames = Vec::new();
        while let Some(f) = mic.read() {
            frames.push(f);
        }
        self.st.step_frames += u32::try_from(frames.len()).unwrap_or(u32::MAX);
        for f in frames {
            match self.dsp.process(&f) {
                Ok(out) => {
                    for p in out {
                        self.on_processed(p);
                    }
                }
                Err(e) => self.degraded("dsp", e.to_string()),
            }
        }
        self.flush_user_push();
    }

    fn on_processed(&mut self, p: Processed) {
        let level = rms_db(&p.frame.pcm);
        self.st.level_db = if level > self.st.level_db {
            level
        } else {
            0.8 * self.st.level_db + 0.2 * level
        };
        let ts = p.frame.ts.as_ms();
        let near = self.echo.near_end(ts, level, p.erle_db, p.aec_confidence);
        let vad_input = if near {
            p.clone()
        } else {
            let silent = vec![0.0f32; p.frame.pcm.len()];
            Processed {
                frame: Frame::mono(silent, p.frame.format.sample_rate, p.frame.ts),
                speech_prob: 0.0,
                speech_likely: false,
                ..p.clone()
            }
        };
        match self.vad.push_processed(&vad_input) {
            Ok(events) => {
                for e in events {
                    self.on_vad(e);
                }
            }
            Err(e) => self.degraded("vad", e.to_string()),
        }
        let keep = (self.cfg.preroll_ms / 10).max(1) as usize;
        self.st.preroll.push_back(p.frame.clone());
        while self.st.preroll.len() > keep {
            self.st.preroll.pop_front();
        }
        self.user_frame(&p.frame);
    }

    /// Zdarzenie VAD → wypowiedź użytkownika, koniec tury, automat, stan mikrofonu.
    pub(crate) fn on_vad(&mut self, event: VadEvent) {
        self.outbox.push(event.to_bus_event());
        match event {
            VadEvent::SpeechStart { ts, .. } => {
                let at_ms = ts.as_ms();
                self.open_user(at_ms);
                self.turn.observe(&TurnEvent::SpeechStart { at_ms });
                self.st.last_activity_ms = self.st.now_ms;
                self.wake_input(WakeInput::Vad { speech: true });
                self.dialog_event(DialogEvent::VadSpeechStart);
            }
            VadEvent::SpeechEnd { ts, .. } => {
                let at_ms = ts.as_ms();
                if let Some(u) = self.st.user.as_mut().filter(|u| !u.closing) {
                    u.vad_active = false;
                    u.speech_end_ms = Some(at_ms);
                }
                self.st.prev_speech_end = Some(at_ms);
                self.turn.observe(&TurnEvent::SpeechEnd { at_ms });
                self.wake_input(WakeInput::Vad { speech: false });
                self.dialog_event(DialogEvent::VadSpeechEnd);
            }
        }
    }
}
