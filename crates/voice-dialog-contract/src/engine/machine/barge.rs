//! Mowa użytkownika w trakcie mowy/myślenia agentki: ducking → potwierdzenie → twardy stop;
//! backchannel nie przerywa; mowa krótsza niż `min_speech_ms` to szum.

use voice_dialog_contract::{
    BargeIn, Command, DialogNotice, DialogPhase, InterruptClassifier, Interruption, UserTurn,
};

use super::Step;
use crate::backchannel::{BackchannelClass, classify};
use crate::prefix::{heard_prefix, unsaid};

impl<C: InterruptClassifier> Step<'_, C> {
    pub(crate) fn on_vad_start(&mut self) {
        self.s.vad_active = true;
        match self.s.phase {
            DialogPhase::Listening => {
                self.s.phase = DialogPhase::UserSpeaking;
                self.s.user = UserTurn {
                    text: String::new(),
                    started_at_ms: Some(self.now),
                };
            }
            DialogPhase::Speaking => self.begin_barge_in(true),
            DialogPhase::Thinking => self.begin_barge_in(false),
            _ => {}
        }
    }

    pub(crate) fn begin_barge_in(&mut self, duck: bool) {
        if self.s.barge_in.is_none() {
            self.s.barge_in = Some(BargeIn {
                started_at_ms: self.now,
                partial: String::new(),
                ducked: false,
            });
        }
        if duck && !self.s.output_ducked {
            self.emit(Command::DuckOutput {
                db: self.m.cfg.duck_db,
            });
            self.s.output_ducked = true;
            self.notify(DialogNotice::Ducked);
            if let Some(b) = self.s.barge_in.as_mut() {
                b.ducked = true;
            }
        }
    }

    pub(crate) fn on_vad_end(&mut self) {
        self.s.vad_active = false;
        if !matches!(self.s.phase, DialogPhase::Speaking | DialogPhase::Thinking) {
            return;
        }
        if let Some(b) = self.s.barge_in.take() {
            let long_enough = self.now.saturating_sub(b.started_at_ms) >= self.m.cfg.min_speech_ms;
            if long_enough
                && classify(&b.partial, &self.m.cfg.backchannel_phrases) == BackchannelClass::Full
            {
                self.notify(DialogNotice::Backchannel { text: b.partial });
            }
            self.restore_output();
        }
    }

    pub(crate) fn restore_output(&mut self) {
        if self.s.output_ducked {
            self.emit(Command::RestoreOutput);
            self.s.output_ducked = false;
            self.notify(DialogNotice::Restored);
        }
    }

    pub(crate) fn on_partial(&mut self, text: &str) {
        match self.s.phase {
            DialogPhase::Listening => {
                self.s.phase = DialogPhase::UserSpeaking;
                let started = self.s.user.started_at_ms.unwrap_or(self.now);
                self.s.user = UserTurn {
                    text: text.to_owned(),
                    started_at_ms: Some(started),
                };
            }
            DialogPhase::UserSpeaking | DialogPhase::Interrupted => {
                text.clone_into(&mut self.s.user.text)
            }
            DialogPhase::Speaking | DialogPhase::Thinking => {
                if let Some(b) = self.s.barge_in.as_mut() {
                    text.clone_into(&mut b.partial);
                }
            }
            DialogPhase::Idle => {}
        }
    }

    /// Timery: potwierdzenie twardego stopu i filler.
    pub(crate) fn timers(&mut self) {
        let cfg = &self.m.cfg;
        let confirm = cfg.effective_confirm_ms();
        let barge = matches!(self.s.phase, DialogPhase::Speaking | DialogPhase::Thinking)
            && self.s.vad_active;
        if let Some(b) = self.s.barge_in.as_ref().filter(|_| barge) {
            let elapsed = self.now.saturating_sub(b.started_at_ms);
            let stop = match classify(&b.partial, &cfg.backchannel_phrases) {
                BackchannelClass::Content => elapsed >= confirm,
                BackchannelClass::Empty => elapsed >= cfg.max_confirm_ms,
                BackchannelClass::Full | BackchannelClass::Prefix => {
                    elapsed >= cfg.backchannel_max_ms
                }
            };
            if stop {
                self.hard_stop(false);
            }
        }
        let filler_due = self.s.phase == DialogPhase::Thinking
            && cfg.fillers
            && !self.s.filler_played
            && self.s.barge_in.is_none()
            && self
                .s
                .thinking_since_ms
                .is_some_and(|t| self.now >= t + cfg.filler_after_ms);
        if filler_due {
            self.emit(Command::PlayFiller);
            self.s.filler_played = true;
        }
    }

    /// Twardy stop bieżącej mowy/myślenia: stop TTS, (anulowanie LLM), czyszczenie kolejki,
    /// zwolnienie głośnika, zapis usłyszanego prefiksu. Faza → `Interrupted`.
    pub(crate) fn hard_stop(&mut self, keep_generation: bool) {
        let barge = self.s.barge_in.take();
        match self.s.phase {
            DialogPhase::Speaking => {
                if let Some(u) = self.s.utterance.clone() {
                    self.emit(Command::StopTts { utterance: u.id });
                    if !keep_generation {
                        self.emit(Command::CancelGeneration);
                    }
                    self.emit(Command::ClearSpeechQueue);
                    let heard = heard_prefix(&u, self.m.cfg.approx_trim);
                    let rest = unsaid(&u, &heard);
                    self.notify(DialogNotice::Interrupted {
                        heard: heard.clone(),
                    });
                    self.s.interruption = Some(Interruption {
                        utterance: u.id,
                        persona: u.persona.clone(),
                        heard,
                        unsaid: rest,
                        at_ms: self.now,
                        intent: None,
                    });
                }
                self.release_speaker();
                self.restore_output();
            }
            DialogPhase::Thinking => {
                if !keep_generation {
                    self.emit(Command::CancelGeneration);
                }
                if self.s.filler_played {
                    self.emit(Command::StopFiller);
                }
                self.s.pending = None;
                self.s.thinking_since_ms = None;
            }
            _ => return,
        }
        self.s.phase = DialogPhase::Interrupted;
        self.s.user = match barge {
            Some(b) => UserTurn {
                text: b.partial,
                started_at_ms: Some(b.started_at_ms),
            },
            None => UserTurn::default(),
        };
    }
}
