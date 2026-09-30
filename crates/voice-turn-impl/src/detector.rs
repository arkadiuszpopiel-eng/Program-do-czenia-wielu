//! `PatienceTurnDetector` — polityka cierpliwości nad VAD, modelem i transkryptem częściowym.

use voice_turn_contract::{
    AudioTail, EndReason, TurnCfg, TurnDecision, TurnDetector, TurnError, TurnEvent, TurnModel,
    TurnModelInput, WaitReason,
};

use crate::hesitation::{ends_clearly, hesitation};

#[derive(Debug, Default, Clone)]
struct State {
    had_speech: bool,
    speaking: bool,
    ended: bool,
    last_end: Option<u64>,
    partial: String,
    partial_version: u64,
    /// (koniec mowy, wersja transkryptu) → wynik modelu (`None` = błąd modelu).
    cached: Option<((u64, u64), Option<f32>)>,
}

/// Detektor końca tury z regulowaną cierpliwością.
#[derive(Debug)]
pub struct PatienceTurnDetector<M: TurnModel> {
    cfg: TurnCfg,
    model: M,
    st: State,
}

impl<M: TurnModel> PatienceTurnDetector<M> {
    /// Detektor z modelem i konfiguracją domyślną.
    pub fn new(model: M) -> Self {
        Self {
            cfg: TurnCfg::default(),
            model,
            st: State::default(),
        }
    }

    /// Detektor z konfiguracją (walidowaną).
    pub fn with_cfg(model: M, cfg: TurnCfg) -> Result<Self, TurnError> {
        cfg.validate()?;
        Ok(Self {
            cfg,
            model,
            st: State::default(),
        })
    }

    /// Model (np. do diagnostyki).
    pub fn model(&self) -> &M {
        &self.model
    }

    fn probability(
        &mut self,
        end: u64,
        silence_ms: u64,
        audio: Option<AudioTail<'_>>,
    ) -> Option<f32> {
        let key = (end, self.st.partial_version);
        if let Some((k, p)) = self.st.cached
            && k == key
        {
            return p;
        }
        let text = self
            .cfg
            .use_partial_text
            .then_some(self.st.partial.as_str())
            .filter(|t| !t.is_empty());
        let input = TurnModelInput {
            audio,
            partial_text: text,
            silence_ms,
        };
        let p = self
            .model
            .end_probability(&input)
            .ok()
            .filter(|p| p.is_finite())
            .map(|p| p.clamp(0.0, 1.0));
        self.st.cached = Some((key, p));
        p
    }
}

impl<M: TurnModel> TurnDetector for PatienceTurnDetector<M> {
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
                self.st.had_speech = true;
                self.st.speaking = true;
                self.st.ended = false;
            }
            TurnEvent::SpeechEnd { at_ms } => {
                // Koniec mowy liczy się tylko po jej początku (duplikaty z VAD są ignorowane).
                if self.st.speaking {
                    self.st.speaking = false;
                    self.st.last_end = Some(*at_ms);
                }
            }
            TurnEvent::Partial { text, .. } => {
                if !self.st.ended && *text != self.st.partial {
                    self.st.partial.clone_from(text);
                    self.st.partial_version += 1;
                }
            }
            TurnEvent::Reset => self.st = State::default(),
        }
    }

    fn decide(&mut self, now_ms: u64, audio: Option<AudioTail<'_>>) -> TurnDecision {
        let p = self.cfg.patience.cfg();
        if !self.st.had_speech || self.st.ended {
            return TurnDecision::Idle;
        }
        let Some(end) = self.st.last_end.filter(|_| !self.st.speaking) else {
            return TurnDecision::Wait {
                until_ms: now_ms + p.min_silence_ms,
                reason: WaitReason::UserSpeaking,
            };
        };
        let silence = now_ms.saturating_sub(end);
        let prob = self.probability(end, silence, audio);
        let text = if self.cfg.use_partial_text {
            self.st.partial.as_str()
        } else {
            ""
        };
        let hes = hesitation(text);
        let clear = ends_clearly(text) && hes.is_none();
        let mut required = p.base_ms;
        let mut reason = WaitReason::Silence;
        match prob {
            Some(v) if v >= self.cfg.confident_threshold => required = p.min_silence_ms,
            Some(v) if v < self.cfg.eot_threshold => {
                required += p.low_prob_bonus_ms;
                reason = WaitReason::ModelUnsure;
            }
            Some(_) => {}
            None if clear => required = p.min_silence_ms,
            None => {}
        }
        if hes.is_some() {
            required = required.max(p.base_ms) + p.hesitation_bonus_ms;
            reason = WaitReason::Hesitation;
        }
        let required = required.clamp(p.min_silence_ms, p.max_ms);
        if silence >= required {
            self.st.ended = true;
            let end_reason = if silence >= p.max_ms && required >= p.max_ms {
                EndReason::MaxSilence
            } else if required <= p.min_silence_ms {
                EndReason::ClearEnd
            } else {
                EndReason::Patience
            };
            let confidence = prob.unwrap_or(if clear { 0.8 } else { 0.6 });
            return TurnDecision::EndOfTurn {
                at_ms: now_ms,
                confidence,
                reason: end_reason,
            };
        }
        TurnDecision::Wait {
            until_ms: end + required,
            reason,
        }
    }
}
