//! Konfiguracja cierpliwości (`[voice.turn]`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::TurnError;

/// Parametry cierpliwości (ms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatienceCfg {
    /// Minimalna cisza przed końcem tury (także przy wyraźnym zakończeniu zdania).
    pub min_silence_ms: u64,
    /// Cisza bazowa.
    pub base_ms: u64,
    /// Wydłużenie po hezytacji („yyy”, „że…”).
    pub hesitation_bonus_ms: u64,
    /// Wydłużenie, gdy model mówi „jeszcze nie skończył”.
    pub low_prob_bonus_ms: u64,
    /// Twardy limit ciszy — po nim zawsze koniec tury (brak zawieszenia).
    pub max_ms: u64,
}

/// Poziom cierpliwości (suwak w UI; „poczekaj dłużej, zanim odpowiesz”).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "level", rename_all = "snake_case")]
pub enum Patience {
    /// Szybkie odpowiedzi.
    Low,
    /// Domyślna.
    Normal,
    /// Dłuższe czekanie (np. dyktowanie myśli).
    High,
    /// Własne wartości.
    Custom(PatienceCfg),
}

impl Patience {
    /// Parametry poziomu.
    pub fn cfg(&self) -> PatienceCfg {
        let p =
            |min_silence_ms, base_ms, hesitation_bonus_ms, low_prob_bonus_ms, max_ms| PatienceCfg {
                min_silence_ms,
                base_ms,
                hesitation_bonus_ms,
                low_prob_bonus_ms,
                max_ms,
            };
        match self {
            Self::Low => p(150, 200, 250, 300, 1000),
            Self::Normal => p(200, 300, 400, 500, 1500),
            Self::High => p(300, 600, 700, 800, 2500),
            Self::Custom(c) => *c,
        }
    }
}

/// Konfiguracja detektora.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TurnCfg {
    /// Cierpliwość.
    pub patience: Patience,
    /// Czy używać transkryptu częściowego (hezytacje, interpunkcja).
    pub use_partial_text: bool,
    /// Próg „model uważa, że koniec” (0–1).
    pub eot_threshold: f32,
    /// Próg „model jest pewny końca” → minimalna cisza.
    pub confident_threshold: f32,
}

impl Default for TurnCfg {
    fn default() -> Self {
        Self {
            patience: Patience::Normal,
            use_partial_text: true,
            eot_threshold: 0.5,
            confident_threshold: 0.85,
        }
    }
}

impl TurnCfg {
    /// Waliduje: `min ≤ base ≤ max`, `max > 0`, progi w (0, 1), pewny ≥ próg.
    pub fn validate(&self) -> Result<(), TurnError> {
        let c = self.patience.cfg();
        let invalid = |reason: &str| {
            Err(TurnError::InvalidConfig {
                reason: reason.to_owned(),
            })
        };
        if c.max_ms == 0 || c.min_silence_ms > c.base_ms || c.base_ms > c.max_ms {
            return invalid("wymagane min_silence ≤ base ≤ max i max > 0");
        }
        let unit = |v: f32| v.is_finite() && v > 0.0 && v < 1.0;
        if !unit(self.eot_threshold)
            || !unit(self.confident_threshold)
            || self.confident_threshold < self.eot_threshold
        {
            return invalid("progi muszą być w (0, 1), a próg pewności ≥ progu końca");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_ordered_and_valid() {
        let (low, normal, high) = (
            Patience::Low.cfg(),
            Patience::Normal.cfg(),
            Patience::High.cfg(),
        );
        assert!(low.max_ms < normal.max_ms && normal.max_ms < high.max_ms);
        assert_eq!(
            (normal.base_ms, normal.hesitation_bonus_ms, normal.max_ms),
            (300, 400, 1500)
        );
        for p in [Patience::Low, Patience::Normal, Patience::High] {
            TurnCfg {
                patience: p,
                ..TurnCfg::default()
            }
            .validate()
            .unwrap();
        }
    }

    #[test]
    fn validation_rejects_bad_values() {
        let bad = PatienceCfg {
            min_silence_ms: 500,
            base_ms: 300,
            hesitation_bonus_ms: 0,
            low_prob_bonus_ms: 0,
            max_ms: 1000,
        };
        assert!(
            TurnCfg {
                patience: Patience::Custom(bad),
                ..TurnCfg::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            TurnCfg {
                eot_threshold: 0.9,
                confident_threshold: 0.8,
                ..TurnCfg::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            TurnCfg {
                eot_threshold: f32::NAN,
                ..TurnCfg::default()
            }
            .validate()
            .is_err()
        );
        let json = serde_json::to_string(&TurnCfg::default()).unwrap();
        assert!(json.contains("\"level\":\"normal\""));
    }
}
