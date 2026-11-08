//! `FakeDialog` — uproszczony, deterministyczny automat do testów UI i `agent-runtime`:
//! mowa użytkownika w `Speaking` = natychmiastowy twardy stop (bez duckingu i backchannelu),
//! prefiks z dokładnością do fragmentu TTS (`approximate`), intencja przerwania zawsze „korekta”.

use voice_dialog_contract::{
    Command, DialogAutomaton, DialogEvent, DialogNotice, DialogPhase, DialogState, PendingSpeech,
    ProactiveRejection, SpokenChunk, Transition, TurnSource, UserTurn, Utterance,
};

use crate::ctx::Ctx;

/// Uproszczony automat rozmowy.
#[derive(Debug, Clone, Copy, Default)]
pub struct FakeDialog;

impl Ctx {
    fn event(&mut self, event: &DialogEvent) {
        use DialogEvent as E;
        let phase = self.s.phase;
        match event {
            E::Activate { .. } if phase == DialogPhase::Idle => {
                if self
                    .s
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.proactive.is_some())
                {
                    self.s.pending = None;
                    self.reject(ProactiveRejection::NotIdle);
                }
                self.out.push(Command::StartListening);
                self.s.phase = DialogPhase::Listening;
            }
            E::Deactivate => {
                self.stop_speaking();
                self.s.pending = None;
                self.out.push(Command::StopListening);
                self.s.phase = DialogPhase::Idle;
            }
            E::IdleTimeout if phase == DialogPhase::Listening => {
                self.out.push(Command::StopListening);
                self.s.phase = DialogPhase::Idle;
            }
            E::VadSpeechStart => {
                self.s.vad_active = true;
                match phase {
                    DialogPhase::Listening => {
                        self.s.user = UserTurn {
                            text: String::new(),
                            started_at_ms: None,
                        };
                        self.s.phase = DialogPhase::UserSpeaking;
                    }
                    DialogPhase::Speaking => {
                        self.stop_speaking();
                        self.s.phase = DialogPhase::Interrupted;
                    }
                    DialogPhase::Thinking => {
                        self.stop_thinking();
                        self.s.phase = DialogPhase::Interrupted;
                    }
                    _ => {}
                }
            }
            E::VadSpeechEnd => self.s.vad_active = false,
            E::UserPartial { text }
                if matches!(
                    phase,
                    DialogPhase::Listening | DialogPhase::UserSpeaking | DialogPhase::Interrupted
                ) =>
            {
                text.clone_into(&mut self.s.user.text);
                if phase == DialogPhase::Listening {
                    self.s.phase = DialogPhase::UserSpeaking;
                }
            }
            E::TurnEnded
                if matches!(phase, DialogPhase::UserSpeaking | DialogPhase::Interrupted) =>
            {
                let text = self.s.user.text.trim().to_owned();
                if text.is_empty() {
                    self.s.phase = DialogPhase::Listening;
                } else {
                    self.submit(&text, TurnSource::Voice, phase == DialogPhase::Interrupted);
                }
            }
            E::Command { command } => self.command(command),
            E::UserTyped { text } => {
                let interrupted = matches!(phase, DialogPhase::Speaking | DialogPhase::Interrupted);
                match phase {
                    DialogPhase::Speaking => self.stop_speaking(),
                    DialogPhase::Thinking => self.stop_thinking(),
                    _ => {}
                }
                self.submit(text.trim(), TurnSource::Text, interrupted);
            }
            E::ResponseReady { persona }
                if phase == DialogPhase::Thinking && self.s.pending.is_none() =>
            {
                let id = self.alloc();
                self.s.pending = Some(PendingSpeech {
                    utterance: id,
                    persona: persona.clone(),
                    text: None,
                    proactive: None,
                });
                self.out.push(Command::AcquireSpeaker {
                    persona: persona.clone(),
                    utterance: id,
                });
            }
            E::SpeakerGranted { persona, utterance } => match self.s.pending.take() {
                Some(p) if p.utterance == *utterance => {
                    self.s.speaker_held = Some(p.utterance);
                    self.s.utterance = Some(Utterance {
                        id: p.utterance,
                        persona: p.persona.clone(),
                        chunks: Vec::new(),
                        played_ms: 0,
                        proactive: p.proactive.clone(),
                    });
                    self.s.phase = DialogPhase::Speaking;
                    self.out.push(match p.proactive {
                        Some(label) => Command::SpeakProactive {
                            utterance: p.utterance,
                            persona: p.persona,
                            text: p.text.unwrap_or_default(),
                            label,
                        },
                        None => Command::StartTts {
                            utterance: p.utterance,
                            persona: p.persona,
                        },
                    });
                }
                other => {
                    self.s.pending = other;
                    self.out.push(Command::ReleaseSpeaker {
                        persona: persona.clone(),
                        utterance: *utterance,
                    });
                }
            },
            E::SpeakerDenied { utterance, .. } => {
                if let Some(p) = self.s.pending.clone().filter(|p| p.utterance == *utterance) {
                    if p.proactive.is_some() {
                        self.s.pending = None;
                        self.reject(ProactiveRejection::SpeakerBusy);
                    } else {
                        self.out.push(Command::Notify {
                            notice: DialogNotice::SpeakerBusy {
                                utterance: *utterance,
                            },
                        });
                    }
                }
            }
            E::SpeakerReleased => {
                if let Some(p) = self.s.pending.clone() {
                    self.out.push(Command::AcquireSpeaker {
                        persona: p.persona,
                        utterance: p.utterance,
                    });
                }
            }
            E::TtsChunkQueued {
                utterance,
                text,
                audio_ms,
            } if self.s.is_speaking(*utterance) => {
                if let Some(u) = self.s.utterance.as_mut() {
                    let (char_start, ms_start) = u.chunks.last().map_or((0, 0), |c| {
                        (
                            c.char_start + c.text.chars().count() + 1,
                            c.ms_start + c.audio_ms,
                        )
                    });
                    u.chunks.push(SpokenChunk {
                        text: text.clone(),
                        audio_ms: *audio_ms,
                        char_start,
                        ms_start,
                        marks: None,
                        mark_source: None,
                    });
                }
            }
            E::PlaybackProgress {
                utterance,
                played_samples,
                sample_rate,
                device_latency_ms,
            } if self.s.is_speaking(*utterance) && *sample_rate > 0 => {
                if let Some(u) = self.s.utterance.as_mut() {
                    u.played_ms = (played_samples * 1000 / u64::from(*sample_rate))
                        .saturating_sub(*device_latency_ms);
                }
            }
            E::ResponseFinished { utterance } if self.s.is_speaking(*utterance) => {
                let proactive = self
                    .s
                    .utterance
                    .as_ref()
                    .is_some_and(|u| u.proactive.is_some());
                self.release();
                self.s.phase = if proactive {
                    DialogPhase::Idle
                } else {
                    DialogPhase::Listening
                };
            }
            E::ProactiveRequest {
                persona,
                text,
                label,
            } => {
                if self.s.do_not_disturb {
                    self.reject(ProactiveRejection::DoNotDisturb);
                } else if phase != DialogPhase::Idle
                    || self.s.pending.is_some()
                    || self.s.vad_active
                {
                    self.reject(ProactiveRejection::NotIdle);
                } else {
                    let id = self.alloc();
                    self.s.pending = Some(PendingSpeech {
                        utterance: id,
                        persona: persona.clone(),
                        text: Some(text.clone()),
                        proactive: Some(label.clone()),
                    });
                    self.out.push(Command::AcquireSpeaker {
                        persona: persona.clone(),
                        utterance: id,
                    });
                }
            }
            E::SetDoNotDisturb { enabled } => self.s.do_not_disturb = *enabled,
            E::StopSpeech => {
                match phase {
                    DialogPhase::Speaking => self.stop_speaking(),
                    DialogPhase::Thinking => self.stop_thinking(),
                    _ => return,
                }
                self.s.phase = DialogPhase::Listening;
            }
            _ => {}
        }
    }
}

impl DialogAutomaton for FakeDialog {
    fn step(&self, state: &DialogState, event: &DialogEvent, _now_ms: u64) -> Transition {
        let mut ctx = Ctx {
            s: state.clone(),
            out: Vec::new(),
        };
        let from = ctx.s.phase;
        ctx.event(event);
        let to = ctx.s.phase;
        if from != to {
            ctx.out.push(Command::Notify {
                notice: DialogNotice::StateChanged { from, to },
            });
        }
        Transition {
            state: ctx.s,
            commands: ctx.out,
        }
    }
}
