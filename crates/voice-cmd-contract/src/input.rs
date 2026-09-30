//! Wejście i wynik rozpoznawania komend.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AgentActivity, VoiceCommand};

/// Token transkryptu ze znacznikami czasu (ms zegara monotonicznego sesji; w testach wirtualnego).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Token {
    /// Tekst tokenu (może zawierać interpunkcję).
    pub text: String,
    /// Początek słowa.
    pub start_ms: u64,
    /// Koniec słowa.
    pub end_ms: u64,
    /// Pewność STT 0–1 (1.0, gdy nieznana).
    pub confidence: f32,
}

impl Token {
    /// Token z pełną pewnością.
    pub fn new(text: impl Into<String>, start_ms: u64, end_ms: u64) -> Self {
        Self {
            text: text.into(),
            start_ms,
            end_ms,
            confidence: 1.0,
        }
    }
}

/// Źródło transkryptu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CmdSource {
    /// Keyword-spotter na audio.
    Kws,
    /// Transkrypt częściowy (STT dwuprzebiegowe).
    Partial,
    /// Transkrypt końcowy.
    Final,
}

/// Wejście rozpoznawania: bieżąca wypowiedź i kontekst.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CmdInput {
    /// Tokeny bieżącej wypowiedzi.
    pub tokens: Vec<Token>,
    /// Źródło.
    pub source: CmdSource,
    /// Aktywność agentki.
    pub activity: AgentActivity,
    /// Bieżący czas.
    pub now_ms: u64,
    /// Koniec poprzedniej mowy użytkownika (do pauzy przed „nie”); `None` = długa cisza.
    pub prev_speech_end_ms: Option<u64>,
    /// Czy wypowiedź jest skierowana do Alfy (PTT, wake-word, tryb rozmowy).
    pub addressed: bool,
}

/// Trafienie komendy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CmdHit {
    /// Komenda.
    pub command: VoiceCommand,
    /// Źródło.
    pub source: CmdSource,
    /// Pewność 0–1.
    pub confidence: f32,
    /// Początek pierwszego słowa komendy (do pomiaru reakcji).
    pub at_ms: u64,
    /// Czy komenda była samodzielna (pauza przed i po).
    pub standalone: bool,
    /// Czy wypowiedź była zaadresowana (flaga wejścia lub imię persony w wypowiedzi).
    pub addressed: bool,
}

/// Powód zignorowania komendy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum IgnoreReason {
    /// Samodzielne „nie” poza stanem `Speaking`.
    NieOutsideSpeaking,
    /// Brak adresata (TV, rozmowa obok) dla komendy innej niż przerwanie mowy.
    NotAddressed,
    /// Pewność poniżej progu.
    LowConfidence {
        /// Pewność.
        confidence: f32,
    },
}

/// Wynik rozpoznawania.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum CmdDecision {
    /// Komenda do wykonania.
    Hit(CmdHit),
    /// Wygląda na komendę, ale trzeba poczekać na pauzę po słowie; sprawdź ponownie o czasie.
    Pending {
        /// Kiedy ponowić ocenę.
        recheck_at_ms: u64,
    },
    /// Rozpoznana, ale nie do wykonania.
    Ignored {
        /// Komenda.
        command: VoiceCommand,
        /// Powód.
        reason: IgnoreReason,
    },
    /// To nie jest komenda (zwykła wypowiedź → LLM).
    NoMatch,
}

impl CmdDecision {
    /// Komenda, jeśli trafienie.
    pub fn hit(&self) -> Option<&CmdHit> {
        match self {
            Self::Hit(hit) => Some(hit),
            _ => None,
        }
    }
}
