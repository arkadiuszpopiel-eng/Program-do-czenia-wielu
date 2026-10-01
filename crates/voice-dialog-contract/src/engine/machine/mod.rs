//! `DialogMachine` — czysto funkcyjny automat rozmowy (PLAN §6.5).
//!
//! `step` klonuje stan, stosuje zdarzenie, uruchamia timery (potwierdzenie twardego stopu, filler)
//! i zwraca nowy stan z poleceniami. Brak I/O, brak zegara systemowego — czas przychodzi z wejścia.

mod barge;
mod speech;
mod turns;

use crate::{
    Command, DialogAutomaton, DialogConfig, DialogEvent, DialogNotice, DialogPhase, DialogState,
    InterruptClassifier, Transition, UserTurn,
};

/// Automat rozmowy z klasyfikatorem intencji przerwania.
#[derive(Debug, Clone)]
pub struct DialogMachine<C> {
    cfg: DialogConfig,
    classifier: C,
}

impl<C: InterruptClassifier> DialogMachine<C> {
    /// Automat z konfiguracją i klasyfikatorem.
    pub fn new(cfg: DialogConfig, classifier: C) -> Self {
        Self { cfg, classifier }
    }

    /// Konfiguracja.
    pub fn config(&self) -> &DialogConfig {
        &self.cfg
    }
}

/// Jeden krok w toku: stan roboczy, polecenia, czas.
pub(crate) struct Step<'a, C> {
    pub m: &'a DialogMachine<C>,
    pub s: DialogState,
    pub out: Vec<Command>,
    pub now: u64,
}

impl<C: InterruptClassifier> Step<'_, C> {
    pub(crate) fn emit(&mut self, c: Command) {
        self.out.push(c);
    }

    pub(crate) fn notify(&mut self, notice: DialogNotice) {
        self.out.push(Command::Notify { notice });
    }

    pub(crate) fn ignored(&mut self, reason: &str) {
        self.notify(DialogNotice::Ignored {
            reason: reason.to_owned(),
        });
    }

    pub(crate) fn clear_user(&mut self) {
        self.s.user = UserTurn::default();
    }

    fn dispatch(&mut self, event: &DialogEvent) {
        use DialogEvent as E;
        match event {
            E::Activate { .. } => self.on_activate(),
            E::Deactivate => {
                self.stop_everything();
                self.emit(Command::StopListening);
                self.s.phase = DialogPhase::Idle;
            }
            E::IdleTimeout => {
                if self.s.phase == DialogPhase::Listening {
                    self.emit(Command::StopListening);
                    self.s.phase = DialogPhase::Idle;
                }
            }
            E::VadSpeechStart => self.on_vad_start(),
            E::VadSpeechEnd => self.on_vad_end(),
            E::UserPartial { text } => self.on_partial(text),
            E::TurnEnded => self.on_turn_ended(),
            E::Command { command } => self.on_command(command),
            E::UserTyped { text } => self.on_typed(text),
            E::ResponseReady { persona } => self.on_response_ready(persona),
            E::SpeakerGranted { persona, utterance } => self.on_granted(persona, *utterance),
            E::SpeakerDenied { utterance, .. } => self.on_denied(*utterance),
            E::SpeakerReleased => self.on_speaker_released(),
            E::TtsChunkQueued {
                utterance,
                text,
                audio_ms,
            } => self.on_chunk(*utterance, text, *audio_ms),
            E::TtsWordMarks {
                utterance,
                chunk,
                marks,
                source,
            } => {
                self.on_marks(*utterance, *chunk, marks, *source);
            }
            E::PlaybackProgress {
                utterance,
                played_samples,
                sample_rate,
                device_latency_ms,
            } => {
                self.on_progress(
                    *utterance,
                    *played_samples,
                    *sample_rate,
                    *device_latency_ms,
                );
            }
            E::ResponseFinished { utterance } => self.on_finished(*utterance),
            E::ProactiveRequest {
                persona,
                text,
                label,
            } => self.on_proactive(persona, text, label),
            E::SetDoNotDisturb { enabled } => {
                self.s.do_not_disturb = *enabled;
                if *enabled {
                    self.drop_proactive_pending();
                }
            }
            E::StopSpeech => self.on_stop_speech(),
            E::Tick => {}
        }
    }

    fn on_activate(&mut self) {
        if self.s.phase != DialogPhase::Idle {
            return;
        }
        self.drop_proactive_pending();
        self.emit(Command::StartListening);
        self.s.phase = DialogPhase::Listening;
    }
}

impl<C: InterruptClassifier> DialogAutomaton for DialogMachine<C> {
    fn step(&self, state: &DialogState, event: &DialogEvent, now_ms: u64) -> Transition {
        let mut st = Step {
            m: self,
            s: state.clone(),
            out: Vec::new(),
            now: now_ms,
        };
        let from = st.s.phase;
        st.dispatch(event);
        st.timers();
        let to = st.s.phase;
        if to != from {
            st.notify(DialogNotice::StateChanged { from, to });
        }
        Transition {
            state: st.s,
            commands: st.out,
        }
    }
}
