//! Wykonywanie poleceń automatu dialogu (`voice-dialog`): ducking, twardy stop, anulowanie
//! generowania, kolejka mówienia, przekazanie tury, wznowienie, fillery, komendy przekazane dalej.
//! Zasób „głośnik” obsługuje sterownik automatu (`DialogDriver` + `SchedSpeakerLock`).

use std::time::Duration;

use personas_contract::PersonaId;
use voice_audio_contract::Ducking;
use voice_cmd_contract::VoiceCommand;
use voice_dialog_contract::{Command, DialogEvent, DialogNotice, DialogPhase, HeardPrefix};
use voice_pipeline_contract::{PipelineEvent, SwitchSource};
use voice_wake_contract::WakeInput;

use crate::pipeline::Pipeline;

/// Krok głośności dla „głośniej/ciszej” (dB).
const VOLUME_STEP_DB: f32 = 3.0;

impl Pipeline {
    /// Zdarzenie automatu → polecenia (w kolejności), z odroczonym anulowaniem generowania (prefiks
    /// usłyszany przychodzi w tym samym kroku automatu po `CancelGeneration`).
    pub(crate) fn dialog_event(&mut self, event: DialogEvent) {
        let before = self.dialog.state().phase;
        let commands = self.dialog.handle(event, self.st.now_ms);
        let heard: Option<HeardPrefix> = commands.iter().find_map(|c| match c {
            Command::Notify {
                notice: DialogNotice::Interrupted { heard },
            } => Some(heard.clone()),
            _ => None,
        });
        let mut cancel = false;
        for c in commands {
            self.st.step_commands += 1;
            if self.cfg.trace_capacity > 0 {
                if self.st.trace.len() >= self.cfg.trace_capacity {
                    self.st.trace.pop_front();
                }
                self.st.seq += 1;
                self.st.trace.push_back(crate::pipeline::TraceEntry {
                    at_ms: self.st.now_ms,
                    seq: self.st.seq,
                    command: c.clone(),
                });
            }
            if matches!(c, Command::CancelGeneration) {
                cancel = true;
                continue;
            }
            self.exec(c);
        }
        if cancel {
            self.cancel_generation(heard.as_ref());
        }
        let after = self.dialog.state().phase;
        if before != after {
            self.wake_processing(after == DialogPhase::Thinking);
            self.st.last_activity_ms = self.st.now_ms;
        }
    }

    /// Stan „przetwarza” w automacie aktywacji (wskaźnik mikrofonu).
    pub(crate) fn wake_processing(&mut self, busy: bool) {
        if self.st.processing != busy {
            self.st.processing = busy;
            self.wake_input(WakeInput::Processing { busy });
        }
    }

    fn exec(&mut self, command: Command) {
        match command {
            Command::StartListening => {
                if !self.wake.is_listening() {
                    self.wake_input(WakeInput::UiToggle);
                }
            }
            Command::StopListening => {
                if self.wake.is_listening() {
                    self.force_stop_listening();
                }
            }
            Command::DuckOutput { db } => {
                let ducking = Ducking {
                    gain_db: db.clamp(-60.0, 0.0),
                    attack: Duration::from_millis(u64::from(self.cfg.duck_attack_ms)),
                };
                if let Err(e) = self.output.duck(ducking) {
                    self.degraded("audio", e.to_string());
                }
            }
            Command::RestoreOutput => {
                let release = Duration::from_millis(u64::from(self.cfg.unduck_release_ms));
                if let Err(e) = self.output.unduck(release) {
                    self.degraded("audio", e.to_string());
                }
            }
            Command::StopTts { utterance } => self.stop_job(utterance.0),
            Command::ClearSpeechQueue => self.clear_speech_queue(),
            Command::CancelTask => self.publish(&PipelineEvent::CancelTask),
            Command::StartTts { utterance, persona } => self.start_job(utterance.0, persona),
            Command::SpeakProactive {
                utterance,
                persona,
                text,
                ..
            } => self.text_job(utterance.0, persona, &text),
            Command::ResumeFrom {
                from,
                offset,
                text,
                utterance,
            } => self.resume_job(from.0, offset, text, utterance.0),
            Command::SubmitTurn {
                turn,
                text,
                interrupted_intent,
                source,
                ..
            } => self.submit_reply(turn.0, text, interrupted_intent, source),
            Command::PlayFiller => self.play_filler(),
            Command::StopFiller => self.stop_fillers(),
            Command::ForwardCommand { command } => self.forward(command),
            Command::KillSwitch => {
                self.publish(&PipelineEvent::KillSwitchRequested);
                if let Err(e) = self.output.stop_all() {
                    self.degraded("audio", e.to_string());
                }
            }
            Command::Notify { notice } => {
                self.outbox.push(notice.to_bus_event());
                if let DialogNotice::Interrupted { heard } = &notice {
                    self.on_interrupted(heard);
                }
            }
            // Zasób głośnika wykonuje sterownik automatu; anulowanie — po partii poleceń.
            Command::AcquireSpeaker { .. }
            | Command::ReleaseSpeaker { .. }
            | Command::CancelGeneration => {}
        }
    }

    fn forward(&mut self, command: VoiceCommand) {
        match command {
            VoiceCommand::VolumeUp => self.publish(&PipelineEvent::Volume {
                step_db: VOLUME_STEP_DB,
            }),
            VoiceCommand::VolumeDown => self.publish(&PipelineEvent::Volume {
                step_db: -VOLUME_STEP_DB,
            }),
            VoiceCommand::MuteMic => self.wake_input(WakeInput::SetMuted { muted: true }),
            VoiceCommand::SwitchPersona { persona } => {
                self.switch_persona(persona, SwitchSource::Command);
            }
            _ => {}
        }
    }

    /// Zmiana mówiącej agentki w locie (głos i persona kolejnej odpowiedzi, bez restartu).
    pub(crate) fn switch_persona(&mut self, persona: PersonaId, by: SwitchSource) {
        if persona == self.st.active_persona {
            return;
        }
        let from = std::mem::replace(&mut self.st.active_persona, persona.clone());
        self.publish(&PipelineEvent::PersonaSwitched {
            from,
            to: persona,
            by,
        });
    }
}
