//! Kontrakt modułu `voice-turn` (PLAN §6.2, §6.4, §6.5; docs/modules/voice-turn/SPEC.md):
//! decyzja o końcu tury użytkownika.
//!
//! Wejście: zdarzenia VAD (początek/koniec mowy z czasem), transkrypt częściowy (hezytacje:
//! „yyy”, „eee”, niedokończone „i…”, „że…”), prawdopodobieństwo końca tury z modelu (`TurnModel`;
//! Smart Turn v3.2 jako impl ONNX przyjdzie później). Wynik: `TurnDecision` — `EndOfTurn` albo
//! `Wait { until_ms }` (następna ocena). Czas: milisekundy zegara monotonicznego (w testach wirtualnego).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod config;
mod decision;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use config::{Patience, PatienceCfg, TurnCfg};
pub use decision::{EndReason, TurnDecision, TurnEvent, WaitReason};

use core_bus_contract::EventKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Koniec tury (pewność, czas od ostatniej ramki mowy).
pub const EVENT_END: &str = "voice.turn.end";
/// Wydłużona cierpliwość po hezytacji.
pub const EVENT_HESITATION: &str = "voice.turn.hesitation";
/// Model końca tury załadowany.
pub const EVENT_MODEL_LOADED: &str = "voice.turn.model_loaded";
/// Model końca tury zwolniony.
pub const EVENT_MODEL_UNLOADED: &str = "voice.turn.model_unloaded";

/// Rodzaj zdarzenia magistrali dla nazwy z tego modułu.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Błędy modułu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum TurnError {
    /// Niepoprawna konfiguracja.
    #[error("niepoprawna konfiguracja końca tury: {reason}")]
    InvalidConfig {
        /// Powód.
        reason: String,
    },
    /// Błąd modelu (np. brak pliku ONNX) — polityka działa wtedy bez modelu.
    #[error("błąd modelu końca tury: {reason}")]
    Model {
        /// Powód.
        reason: String,
    },
}

/// Ogon audio po VAD (ostatnie ~8 s) dla modelu.
#[derive(Debug, Clone, Copy)]
pub struct AudioTail<'a> {
    /// Próbki mono f32.
    pub samples: &'a [f32],
    /// Częstotliwość próbkowania.
    pub sample_rate: u32,
}

/// Wejście modelu końca tury.
#[derive(Debug, Clone, Copy)]
pub struct TurnModelInput<'a> {
    /// Audio (jeśli dostępne).
    pub audio: Option<AudioTail<'a>>,
    /// Transkrypt częściowy (jeśli dostępny).
    pub partial_text: Option<&'a str>,
    /// Cisza od ostatniej ramki mowy.
    pub silence_ms: u64,
}

/// Model końca tury (Smart Turn v3.2 ONNX, heurystyka tekstowa, atrapa).
pub trait TurnModel: Send + Sync {
    /// Nazwa (do zdarzeń i diagnostyki).
    fn name(&self) -> &str;
    /// Prawdopodobieństwo, że użytkownik skończył turę (0–1).
    fn end_probability(&self, input: &TurnModelInput<'_>) -> Result<f32, TurnError>;
}

/// Detektor końca tury (stanowy, deterministyczny; czas podaje wywołujący).
pub trait TurnDetector: Send {
    /// Zmienia konfigurację (walidowaną).
    fn configure(&mut self, cfg: TurnCfg) -> Result<(), TurnError>;
    /// Bieżąca konfiguracja.
    fn config(&self) -> &TurnCfg;
    /// Przyjmuje zdarzenie VAD / transkryptu.
    fn observe(&mut self, event: &TurnEvent);
    /// Decyzja na chwilę `now_ms` (wywoływana po `SpeechEnd` i cyklicznie w ciszy).
    fn decide(&mut self, now_ms: u64, audio: Option<AudioTail<'_>>) -> TurnDecision;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_and_errors() {
        for name in [
            EVENT_END,
            EVENT_HESITATION,
            EVENT_MODEL_LOADED,
            EVENT_MODEL_UNLOADED,
        ] {
            assert_eq!(event_kind(name).to_string(), name);
        }
        let json = serde_json::to_value(TurnError::Model { reason: "x".into() }).unwrap();
        assert_eq!(json["error"], "model");
    }
}
