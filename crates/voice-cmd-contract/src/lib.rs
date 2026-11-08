//! Kontrakt modułu `voice-cmd` (PLAN §6.2, §6.5; docs/modules/voice-cmd/SPEC.md): szybka ścieżka
//! komend głosowych bez LLM.
//!
//! Wejście: tokeny transkryptu ze znacznikami czasu (partial/final) + aktywność agentki
//! (`AgentActivity`, mapowana ze stanu `voice-dialog`, żeby uniknąć cyklu zależności).
//! Wyjście: `CmdDecision` (trafienie z pewnością, oczekiwanie na pauzę, zignorowanie, brak).
//! Gramatyka PL/EN jest edytowalna (`Grammar`, serde; pierścień R0). Deterministyczny rdzeń
//! [`GrammarRecognizer`] (wzorce, tolerancja szumu ASR) jest w kontrakcie — `voice-cmd-impl` go
//! udostępnia, a runnery ewaluacji (zestaw F2) mogą go użyć bez zależności od `-impl`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod command;
mod engine;
mod grammar;
mod input;
mod text;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use command::{AgentActivity, CommandKind, CommandSafety, VoiceCommand};
pub use engine::{EXACT, GrammarRecognizer, ONE_EDIT, TWO_EDITS, levenshtein, word_score};
pub use grammar::{Grammar, GrammarRule, NieRule, PersonaForms};
pub use input::{CmdDecision, CmdHit, CmdInput, CmdSource, IgnoreReason, Token};
pub use text::{fold, nie_verdict, split_tokens};

use core_bus_contract::EventKind;

/// Wykryto komendę (komenda, źródło, opóźnienie).
pub const EVENT_DETECTED: &str = "voice.cmd.detected";
/// Komendę zignorowano (np. „nie” poza `Speaking`, brak adresata).
pub const EVENT_IGNORED: &str = "voice.cmd.ignored";
/// Komendę przekazano do Brokera (zmiana uprawnień — w F3).
pub const EVENT_FORWARDED_TO_BROKER: &str = "voice.cmd.forwarded_to_broker";

/// Rodzaj zdarzenia magistrali dla nazwy z tego modułu.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Rozpoznawanie komend (reguły na partial/final; KWS audio przyjdzie jako osobna implementacja).
pub trait CommandRecognizer: Send + Sync {
    /// Ocenia bieżący transkrypt wypowiedzi.
    fn recognize(&self, input: &CmdInput) -> CmdDecision;
    /// Bieżąca gramatyka (do ściągawki w UI i edytora).
    fn grammar(&self) -> Grammar;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_and_schema() {
        for name in [EVENT_DETECTED, EVENT_IGNORED, EVENT_FORWARDED_TO_BROKER] {
            assert!(name.starts_with("voice.cmd."));
            assert_eq!(event_kind(name).to_string(), name);
        }
        let schema = serde_json::to_string(&schemars::schema_for!(CmdDecision)).unwrap();
        assert!(schema.contains("recheck_at_ms"));
    }
}
