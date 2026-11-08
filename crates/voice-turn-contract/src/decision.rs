//! Zdarzenia wejściowe i decyzje.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Zdarzenie wejściowe detektora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TurnEvent {
    /// VAD: początek mowy.
    SpeechStart {
        /// Czas.
        at_ms: u64,
    },
    /// VAD: koniec mowy (ostatnia ramka mowy).
    SpeechEnd {
        /// Czas.
        at_ms: u64,
    },
    /// Transkrypt częściowy całej bieżącej tury.
    Partial {
        /// Czas.
        at_ms: u64,
        /// Tekst.
        text: String,
    },
    /// Nowa tura (np. po odpowiedzi agentki) — zapomnij stan.
    Reset,
}

/// Dlaczego czekamy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaitReason {
    /// Użytkownik mówi.
    UserSpeaking,
    /// Zwykła cisza — jeszcze za krótka.
    Silence,
    /// Hezytacja — cierpliwość wydłużona.
    Hesitation,
    /// Model uważa, że to jeszcze nie koniec.
    ModelUnsure,
}

/// Dlaczego koniec tury.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// Wyraźne zakończenie (model pewny lub interpunkcja końcowa) po minimalnej ciszy.
    ClearEnd,
    /// Upłynęła wymagana cisza.
    Patience,
    /// Twardy limit ciszy.
    MaxSilence,
}

/// Decyzja detektora.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum TurnDecision {
    /// Brak tury do oceny (nikt nie mówił albo tura już zakończona).
    Idle,
    /// Czekaj; oceń ponownie najpóźniej o `until_ms`.
    Wait {
        /// Kiedy ponowić ocenę.
        until_ms: u64,
        /// Powód.
        reason: WaitReason,
    },
    /// Koniec tury.
    EndOfTurn {
        /// Czas decyzji.
        at_ms: u64,
        /// Pewność 0–1.
        confidence: f32,
        /// Powód.
        reason: EndReason,
    },
}
