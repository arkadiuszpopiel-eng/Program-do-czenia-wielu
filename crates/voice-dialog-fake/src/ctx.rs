//! Stan roboczy atrapy i operacje pomocnicze (stop, tura, komendy).

use voice_cmd_contract::VoiceCommand;
use voice_dialog_contract::{
    Command, DialogNotice, DialogPhase, DialogState, HeardPrefix, InterruptIntent, Interruption,
    PrefixSource, ProactiveRejection, TurnId, TurnSource, UserTurn, UtteranceId,
};

pub(crate) struct Ctx {
    pub(crate) s: DialogState,
    pub(crate) out: Vec<Command>,
}

impl Ctx {
    pub(crate) fn alloc(&mut self) -> UtteranceId {
        self.s.next_utterance += 1;
        UtteranceId(self.s.next_utterance)
    }

    pub(crate) fn release(&mut self) {
        if let (Some(id), Some(u)) = (self.s.speaker_held.take(), self.s.utterance.as_ref()) {
            self.out.push(Command::ReleaseSpeaker {
                persona: u.persona.clone(),
                utterance: id,
            });
        }
    }

    pub(crate) fn stop_speaking(&mut self) {
        let Some(u) = self
            .s
            .utterance
            .clone()
            .filter(|_| self.s.phase == DialogPhase::Speaking)
        else {
            return;
        };
        self.out.push(Command::StopTts { utterance: u.id });
        self.out.push(Command::CancelGeneration);
        self.out.push(Command::ClearSpeechQueue);
        let full = u.full_text();
        let chars = u
            .chunks
            .iter()
            .filter(|c| c.ms_start + c.audio_ms <= u.played_ms)
            .map(|c| c.char_start + c.text.chars().count())
            .max()
            .unwrap_or(0);
        let text: String = full.chars().take(chars).collect();
        let heard = HeardPrefix {
            utterance: u.id,
            chars,
            words: text.split_whitespace().count(),
            text,
            approximate: true,
            source: if chars == 0 {
                PrefixSource::NothingPlayed
            } else {
                PrefixSource::SampleCount
            },
        };
        self.out.push(Command::Notify {
            notice: DialogNotice::Interrupted {
                heard: heard.clone(),
            },
        });
        let unsaid = full
            .chars()
            .skip(chars)
            .collect::<String>()
            .trim_start()
            .to_owned();
        self.s.interruption = Some(Interruption {
            utterance: u.id,
            persona: u.persona.clone(),
            heard,
            unsaid,
            at_ms: 0,
            intent: None,
        });
        self.release();
    }

    pub(crate) fn stop_thinking(&mut self) {
        self.out.push(Command::CancelGeneration);
        self.s.pending = None;
    }

    pub(crate) fn submit(&mut self, text: &str, source: TurnSource, interrupted: bool) {
        self.s.turns += 1;
        let heard = self
            .s
            .interruption
            .take()
            .filter(|_| interrupted)
            .map(|i| i.heard);
        self.out.push(Command::SubmitTurn {
            turn: TurnId(self.s.turns),
            text: text.to_owned(),
            interrupted_intent: heard.as_ref().map(|_| InterruptIntent::Correction),
            heard_prefix: heard,
            source,
        });
        self.s.user = UserTurn::default();
        self.s.phase = DialogPhase::Thinking;
    }

    pub(crate) fn reject(&mut self, reason: ProactiveRejection) {
        self.out.push(Command::Notify {
            notice: DialogNotice::ProactiveRejected { reason },
        });
    }

    pub(crate) fn command(&mut self, c: &VoiceCommand) {
        match c {
            VoiceCommand::Stop
            | VoiceCommand::Cancel
            | VoiceCommand::Wait
            | VoiceCommand::Pause
            | VoiceCommand::No => {
                match self.s.phase {
                    DialogPhase::Speaking => self.stop_speaking(),
                    DialogPhase::Thinking => self.stop_thinking(),
                    _ => {}
                }
                self.s.phase = if *c == VoiceCommand::No {
                    DialogPhase::Interrupted
                } else {
                    DialogPhase::Listening
                };
                if *c == VoiceCommand::Cancel {
                    self.out.push(Command::CancelTask);
                }
            }
            VoiceCommand::StopAll => {
                self.stop_speaking();
                self.s.pending = None;
                self.out.push(Command::KillSwitch);
                self.s.phase = DialogPhase::Idle;
            }
            VoiceCommand::DoNotDisturb => self.s.do_not_disturb = true,
            VoiceCommand::Resume | VoiceCommand::Repeat => {
                self.out.push(Command::Notify {
                    notice: DialogNotice::Ignored {
                        reason: "atrapa nie wznawia".into(),
                    },
                });
            }
            other => self.out.push(Command::ForwardCommand {
                command: other.clone(),
            }),
        }
    }
}
