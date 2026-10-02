//! Krok wątku przetwarzania (~10 ms) i pętla `run`. Kolejność: wejścia UI → aktywacja → wyniki
//! asynchroniczne → wyjście (referencja AEC, raporty odtwarzania, postęp) → mikrofon → timery →
//! wyniki zleceń z tego kroku → zdarzenia modułów → pigułka/rezydencja → magistrala.

use std::task::{Context, Poll};
use std::time::Duration;

use async_trait::async_trait;
use providers_contract::CancellationToken;
use voice_audio_contract::AudioEvent;
use voice_dialog_contract::{DialogEvent, DialogPhase, SpeakerLock};
use voice_pipeline_contract::{
    PipelineEvent, PipelineInput, PipelineStatus, Speaker, StepReport, VoicePipeline,
};

use crate::pipeline::Pipeline;

impl Pipeline {
    fn step_with(&mut self, cx: &mut Context<'_>) -> StepReport {
        self.st.now_ms = self.st.now_ms.max(self.clock.now().as_ms());
        self.st.step_commands = 0;
        self.st.step_frames = 0;
        self.process_inputs();
        let wake = self.wake.pump();
        self.on_wake_events(wake);
        self.poll_async(cx);
        self.process_output();
        if self.wake_words.is_some() {
            self.process_wake_words();
        }
        if !self.wake_idle() {
            self.process_mic();
        }
        self.timers();
        self.poll_async(cx);
        self.forward_module_events();
        self.housekeeping();
        let events = self.outbox.poll(cx);
        StepReport {
            now_ms: self.st.now_ms,
            mic_frames: self.st.step_frames,
            commands: self.st.step_commands,
            events,
        }
    }

    fn poll_async(&mut self, cx: &mut Context<'_>) {
        for out in self.stt.poll(cx) {
            self.on_stt_out(out);
        }
        self.poll_speaker();
        self.poll_reply(cx);
        self.poll_jobs(cx);
    }

    fn process_output(&mut self) {
        for frame in self.output.drain_reference() {
            self.dsp.push_reference(&frame);
            self.echo.push_reference(&frame);
        }
        for event in self.output.poll_events() {
            match &event {
                AudioEvent::PlaybackStarted { utterance, .. } => self.note_first_audio(*utterance),
                AudioEvent::PlaybackFinished {
                    utterance, stopped, ..
                } => {
                    self.note_first_audio(*utterance);
                    self.on_playback_finished(*utterance, *stopped);
                }
                _ => {}
            }
            self.outbox.push(event.to_bus_event());
        }
        let speaking = {
            let s = self.dialog.state();
            s.utterance
                .as_ref()
                .filter(|_| s.phase == DialogPhase::Speaking)
                .map(|u| u.id)
        };
        if let Some(id) = speaking {
            self.note_first_audio(id.0);
            if let Some(pos) = self.output.position(id.0) {
                self.dialog_event(DialogEvent::PlaybackProgress {
                    utterance: id,
                    played_samples: pos.rendered_samples,
                    sample_rate: pos.sample_rate,
                    device_latency_ms: u64::try_from(pos.output_latency.as_millis()).unwrap_or(0),
                });
            }
        }
    }

    fn timers(&mut self) {
        self.user_timers();
        if self.speaker.lost() && self.dialog.state().phase == DialogPhase::Speaking {
            self.degraded("speaker", "głośnik odebrany przez scheduler".into());
            self.dialog_event(DialogEvent::StopSpeech);
        }
        if self.dialog.state().pending.is_some()
            && SpeakerLock::holder(self.speaker.as_ref()).is_none()
        {
            self.dialog_event(DialogEvent::SpeakerReleased);
        }
        if self.mic_lease.revoked() {
            self.mic_lease.release();
            self.degraded("mic", "mikrofon odebrany przez scheduler".into());
            self.force_stop_listening();
        }
        let idle = self.cfg.idle_timeout_ms;
        if idle > 0
            && self.dialog.state().phase == DialogPhase::Listening
            && self.st.user.is_none()
            && self.st.now_ms.saturating_sub(self.st.last_activity_ms) >= idle
        {
            self.dialog_event(DialogEvent::IdleTimeout);
        }
        self.dialog_event(DialogEvent::Tick);
    }

    fn forward_module_events(&mut self) {
        for e in self.stt.take_events() {
            self.outbox.push(e.to_bus_event());
        }
        for e in self.tts.take_events() {
            self.outbox.push(e.to_bus_event());
        }
        for e in self.dsp.take_events() {
            self.outbox.push(e.to_bus_event());
        }
    }

    fn speaker_now(&self) -> Speaker {
        let s = self.dialog.state();
        match s.utterance.as_ref() {
            Some(u) if s.phase == DialogPhase::Speaking => Speaker::Agent(u.persona.clone()),
            _ if s.vad_active => Speaker::User,
            _ => Speaker::Nobody,
        }
    }

    fn housekeeping(&mut self) {
        self.publish_latency();
        let phase = self.dialog.state().phase;
        // Sam nasłuch słów wywoławczych nie trzyma modeli STT/TTS w pamięci.
        let active = (self.mic.is_some() && !self.wake_idle())
            || !matches!(phase, DialogPhase::Idle | DialogPhase::Listening);
        self.residency.update(active, self.st.now_ms);
        let speaker = self.speaker_now();
        let due = self
            .st
            .last_pill_ms
            .is_none_or(|t| self.st.now_ms.saturating_sub(t) >= u64::from(self.cfg.pill_every_ms));
        if speaker != self.st.last_speaker || (due && active) {
            self.st.last_pill_ms = Some(self.st.now_ms);
            self.st.last_speaker = speaker.clone();
            let event = PipelineEvent::Pill {
                speaker,
                persona: self.st.active_persona.clone(),
                level_db: self.st.level_db.round(),
                phase,
                mic: self.wake.mic_state(),
            };
            self.publish(&event);
        }
    }

    /// Pętla wątku przetwarzania (produkcyjnie): krok co `tick_ms`, do anulowania `shutdown`.
    pub async fn run(mut self, shutdown: CancellationToken) {
        let mut tick = tokio::time::interval(Duration::from_millis(u64::from(self.cfg.tick_ms)));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                _ = tick.tick() => {
                    self.step().await;
                }
            }
        }
        self.dialog_event(DialogEvent::Deactivate);
    }
}

#[async_trait]
impl VoicePipeline for Pipeline {
    fn input(&mut self, input: PipelineInput) {
        self.st.inputs.push_back(input);
    }

    async fn step(&mut self) -> StepReport {
        std::future::poll_fn(|cx| Poll::Ready(self.step_with(cx))).await
    }

    fn status(&self) -> PipelineStatus {
        let s = self.dialog.state();
        PipelineStatus {
            now_ms: self.st.now_ms,
            phase: s.phase,
            mic: self.wake.mic_state(),
            mic_open: self.mic.is_some(),
            speaker: self.speaker_now(),
            persona: self.st.active_persona.clone(),
            level_db: self.st.level_db,
            partial: self.st.partial.clone(),
            heard_prefix: self.st.heard.clone(),
            turns: self.st.turns,
            interruptions: self.st.interruptions,
            echo_gated_frames: self.echo.gated(),
            latency: self.st.latency,
        }
    }
}
