//! Tury użytkownika: koniec tury, przerwanie tekstem, komendy głosowe, klasyfikacja intencji,
//! stop mowy (`Esc`) i stop wszystkiego (kill-switch).

use voice_cmd_contract::VoiceCommand;
use voice_dialog_contract::{
    Command, DialogNotice, DialogPhase, HeardPrefix, InterruptClassifier, InterruptContext,
    InterruptIntent, TurnId, TurnSource,
};

use super::Step;

impl<C: InterruptClassifier> Step<'_, C> {
    /// Przekazuje turę do LLM; faza → `Thinking`.
    pub(crate) fn submit(
        &mut self,
        text: &str,
        heard: Option<HeardPrefix>,
        intent: Option<InterruptIntent>,
        source: TurnSource,
    ) {
        self.s.turns += 1;
        self.emit(Command::SubmitTurn {
            turn: TurnId(self.s.turns),
            text: text.to_owned(),
            heard_prefix: heard,
            interrupted_intent: intent,
            source,
        });
        self.s.phase = DialogPhase::Thinking;
        self.s.thinking_since_ms = Some(self.now);
        self.s.filler_played = false;
        self.s.interruption = None;
        self.clear_user();
    }

    pub(crate) fn on_turn_ended(&mut self) {
        match self.s.phase {
            DialogPhase::UserSpeaking => {
                let text = self.s.user.text.trim().to_owned();
                if text.is_empty() {
                    self.clear_user();
                    self.s.phase = DialogPhase::Listening;
                } else {
                    self.submit(&text, None, None, TurnSource::Voice);
                }
            }
            DialogPhase::Interrupted => {
                let text = self.s.user.text.clone();
                self.finish_interruption(&text, TurnSource::Voice);
            }
            _ => {}
        }
    }

    /// Po przerwaniu: klasyfikacja intencji i reakcja (tura / wznowienie / stop).
    pub(crate) fn finish_interruption(&mut self, text: &str, source: TurnSource) {
        let text = text.trim().to_owned();
        self.clear_user();
        if text.is_empty() {
            self.ignored("przerwanie bez treści");
            self.s.phase = DialogPhase::Listening;
            return;
        }
        let intr = self.s.interruption.clone();
        let (heard, rest) = intr
            .as_ref()
            .map_or(("", ""), |i| (i.heard.text.as_str(), i.unsaid.as_str()));
        let r = self.m.classifier.classify(&InterruptContext {
            heard_prefix: heard,
            unsaid: rest,
            utterance: &text,
        });
        self.notify(DialogNotice::IntentClassified {
            intent: r.intent,
            confidence: r.confidence,
        });
        if let Some(i) = self.s.interruption.as_mut() {
            i.intent = Some(r.intent);
        }
        match (r.intent, intr) {
            (InterruptIntent::StopCancel, _) => self.s.phase = DialogPhase::Listening,
            (InterruptIntent::Continue | InterruptIntent::Backchannel, Some(i)) => {
                self.resume(i.utterance, i.persona, i.heard.chars, i.unsaid);
            }
            (InterruptIntent::Continue | InterruptIntent::Backchannel, None) => {
                self.s.phase = DialogPhase::Listening;
            }
            (intent, i) => self.submit(&text, i.map(|i| i.heard), Some(intent), source),
        }
    }

    pub(crate) fn on_typed(&mut self, text: &str) {
        match self.s.phase {
            DialogPhase::Speaking | DialogPhase::Thinking | DialogPhase::Interrupted => {
                if self.s.phase != DialogPhase::Interrupted {
                    self.hard_stop(false);
                }
                self.finish_interruption(text, TurnSource::Text);
            }
            _ => {
                if !text.trim().is_empty() {
                    self.submit(text.trim(), None, None, TurnSource::Text);
                }
            }
        }
    }

    /// `Esc` / „stop mowy”: zatrzymuje mowę (i generowanie mówionej odpowiedzi), nie zabija pracy w tle.
    pub(crate) fn on_stop_speech(&mut self) {
        match self.s.phase {
            DialogPhase::Speaking | DialogPhase::Thinking => {
                self.hard_stop(false);
                self.mark_intent(InterruptIntent::StopCancel);
                self.clear_user();
                self.s.phase = DialogPhase::Listening;
            }
            _ => self.ignored("brak mowy do zatrzymania"),
        }
    }

    fn mark_intent(&mut self, intent: InterruptIntent) {
        if let Some(i) = self.s.interruption.as_mut() {
            i.intent = Some(intent);
        }
    }

    /// Zatrzymuje wszystko (kill-switch / wyjście z trybu głosowego).
    pub(crate) fn stop_everything(&mut self) {
        if self.s.phase == DialogPhase::Speaking
            && let Some(id) = self.s.utterance.as_ref().map(|u| u.id)
        {
            self.emit(Command::StopTts { utterance: id });
        }
        if matches!(self.s.phase, DialogPhase::Speaking | DialogPhase::Thinking)
            || self.s.pending.is_some()
        {
            self.emit(Command::CancelGeneration);
        }
        if self.s.phase == DialogPhase::Thinking && self.s.filler_played {
            self.emit(Command::StopFiller);
        }
        self.emit(Command::ClearSpeechQueue);
        self.release_speaker();
        self.restore_output();
        self.s.pending = None;
        self.s.barge_in = None;
        self.s.thinking_since_ms = None;
        self.clear_user();
    }

    pub(crate) fn on_command(&mut self, command: &VoiceCommand) {
        match command {
            VoiceCommand::Stop | VoiceCommand::Cancel => {
                if matches!(self.s.phase, DialogPhase::Speaking | DialogPhase::Thinking) {
                    self.on_stop_speech();
                } else if self.s.phase == DialogPhase::Interrupted {
                    self.clear_user();
                    self.s.phase = DialogPhase::Listening;
                }
                if *command == VoiceCommand::Cancel {
                    self.emit(Command::CancelTask);
                }
            }
            VoiceCommand::Wait | VoiceCommand::Pause => {
                if self.s.phase == DialogPhase::Speaking {
                    // Pauza: generowanie trwa dalej, żeby „wznów” miało z czego kontynuować.
                    self.hard_stop(true);
                    self.clear_user();
                    self.s.phase = DialogPhase::Listening;
                } else {
                    self.ignored("pauza poza mową");
                }
            }
            VoiceCommand::No => {
                if self.s.phase == DialogPhase::Speaking {
                    self.hard_stop(false);
                } else {
                    self.ignored("„nie” poza mową agentki");
                }
            }
            VoiceCommand::Resume => self.on_resume_command(),
            VoiceCommand::Repeat => self.on_repeat(),
            VoiceCommand::StopAll => {
                self.stop_everything();
                self.emit(Command::KillSwitch);
                self.s.phase = DialogPhase::Idle;
            }
            VoiceCommand::DoNotDisturb => {
                self.s.do_not_disturb = true;
                self.drop_proactive_pending();
            }
            VoiceCommand::MuteMic => {
                self.emit(Command::ForwardCommand {
                    command: command.clone(),
                });
                if matches!(
                    self.s.phase,
                    DialogPhase::Listening | DialogPhase::UserSpeaking
                ) {
                    self.emit(Command::StopListening);
                    self.clear_user();
                    self.s.phase = DialogPhase::Idle;
                }
            }
            VoiceCommand::VolumeUp
            | VoiceCommand::VolumeDown
            | VoiceCommand::SwitchPersona { .. } => {
                self.emit(Command::ForwardCommand {
                    command: command.clone(),
                });
            }
        }
    }

    fn on_resume_command(&mut self) {
        let can = matches!(
            self.s.phase,
            DialogPhase::Listening | DialogPhase::Interrupted | DialogPhase::Idle
        );
        match self.s.interruption.clone().filter(|_| can) {
            Some(i) => {
                self.clear_user();
                self.resume(i.utterance, i.persona, i.heard.chars, i.unsaid);
            }
            None => self.ignored("brak przerwanej wypowiedzi do wznowienia"),
        }
    }

    fn on_repeat(&mut self) {
        if matches!(
            self.s.phase,
            DialogPhase::Thinking | DialogPhase::UserSpeaking
        ) {
            self.ignored("powtórz w trakcie tury");
            return;
        }
        if self.s.phase == DialogPhase::Speaking {
            self.hard_stop(true);
        }
        match self.s.utterance.clone() {
            Some(u) => {
                self.clear_user();
                self.resume(u.id, u.persona.clone(), 0, u.full_text());
            }
            None => self.ignored("nie ma czego powtórzyć"),
        }
    }
}
