//! Atrapy modułu `voice-turn` (SPEC „Fake”): `ScriptedTurnModel` (prawdopodobieństwa lub błędy
//! z kolejki) i `ScriptedTurnDetector` (koniec tury po adnotowanej ciszy — do deterministycznych
//! testów `voice-dialog`). Czas podaje wywołujący (wirtualny zegar).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use voice_turn_contract::{
    AudioTail, EndReason, TurnCfg, TurnDecision, TurnDetector, TurnError, TurnEvent, TurnModel,
    TurnModelInput, WaitReason,
};

#[derive(Debug, Default)]
struct ModelState {
    queue: VecDeque<Result<f32, TurnError>>,
    calls: Vec<Option<String>>,
}

/// Model końca tury ze skryptu: kolejne wywołania zwracają kolejne wartości, potem `default`.
#[derive(Debug)]
pub struct ScriptedTurnModel {
    default: f32,
    state: Mutex<ModelState>,
}

impl ScriptedTurnModel {
    /// Model zwracający zawsze `default` (po wyczerpaniu kolejki).
    pub fn constant(default: f32) -> Self {
        Self {
            default,
            state: Mutex::new(ModelState::default()),
        }
    }

    /// Dokłada wynik do kolejki.
    pub fn push(&self, result: Result<f32, TurnError>) {
        self.lock().queue.push_back(result);
    }

    /// Transkrypty przekazane w kolejnych wywołaniach.
    pub fn calls(&self) -> Vec<Option<String>> {
        self.lock().calls.clone()
    }

    fn lock(&self) -> MutexGuard<'_, ModelState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl TurnModel for ScriptedTurnModel {
    fn name(&self) -> &str {
        "scripted"
    }

    fn end_probability(&self, input: &TurnModelInput<'_>) -> Result<f32, TurnError> {
        let mut st = self.lock();
        st.calls.push(input.partial_text.map(str::to_owned));
        st.queue.pop_front().unwrap_or(Ok(self.default))
    }
}

/// Detektor z adnotacji: koniec tury po `delay` ms ciszy (kolejne tury — kolejne adnotacje),
/// przycięte do `[min_silence_ms, max_ms]`; bez adnotacji — `base_ms`.
#[derive(Debug, Default)]
pub struct ScriptedTurnDetector {
    cfg: TurnCfg,
    delays: VecDeque<u64>,
    had_speech: bool,
    speaking: bool,
    ended: bool,
    last_end: Option<u64>,
    decisions: u32,
}

impl ScriptedTurnDetector {
    /// Detektor z konfiguracją domyślną.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adnotacja: następna tura kończy się po `delay_ms` ciszy.
    pub fn push_delay(&mut self, delay_ms: u64) {
        self.delays.push_back(delay_ms);
    }

    /// Liczba wywołań `decide`.
    pub fn decisions(&self) -> u32 {
        self.decisions
    }
}

impl TurnDetector for ScriptedTurnDetector {
    fn configure(&mut self, cfg: TurnCfg) -> Result<(), TurnError> {
        cfg.validate()?;
        self.cfg = cfg;
        Ok(())
    }

    fn config(&self) -> &TurnCfg {
        &self.cfg
    }

    fn observe(&mut self, event: &TurnEvent) {
        match event {
            TurnEvent::SpeechStart { .. } => {
                self.had_speech = true;
                self.speaking = true;
                self.ended = false;
            }
            TurnEvent::SpeechEnd { at_ms } => {
                if self.speaking {
                    self.speaking = false;
                    self.last_end = Some(*at_ms);
                }
            }
            TurnEvent::Partial { .. } => {}
            TurnEvent::Reset => {
                self.had_speech = false;
                self.speaking = false;
                self.ended = false;
                self.last_end = None;
            }
        }
    }

    fn decide(&mut self, now_ms: u64, _audio: Option<AudioTail<'_>>) -> TurnDecision {
        self.decisions += 1;
        let p = self.cfg.patience.cfg();
        if !self.had_speech || self.ended {
            return TurnDecision::Idle;
        }
        let Some(end) = self.last_end.filter(|_| !self.speaking) else {
            return TurnDecision::Wait {
                until_ms: now_ms + p.min_silence_ms,
                reason: WaitReason::UserSpeaking,
            };
        };
        let delay = self
            .delays
            .front()
            .copied()
            .unwrap_or(p.base_ms)
            .clamp(p.min_silence_ms, p.max_ms);
        if now_ms >= end + delay {
            self.ended = true;
            self.delays.pop_front();
            return TurnDecision::EndOfTurn {
                at_ms: now_ms,
                confidence: 1.0,
                reason: EndReason::Patience,
            };
        }
        TurnDecision::Wait {
            until_ms: end + delay,
            reason: WaitReason::Silence,
        }
    }
}
