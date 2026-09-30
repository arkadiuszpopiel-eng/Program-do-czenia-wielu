//! Kontrakt modułu `memory` v0 (docs/modules/memory/SPEC.md, PLAN §10, ADR 0008).
//!
//! v0 obsługuje wyłącznie zakres [`MemoryScope::Session`] i warstwy epizodyczną/semantyczną;
//! pozostałe zakresy i warstwy są w typach, a implementacja zwraca [`MemoryError::Unsupported`].
//! Wpisy z treści niezaufanej ([`Provenance::UntrustedContent`]) są oznaczone, nie mogą być
//! zapamiętane automatycznie ani awansować do innego zakresu. `forget` jest kaskadowe
//! (wpis + FTS + wektor) i zwraca raport ([`ForgetReport`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod rules;
mod types;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use rules::{check_promotion, is_expired, recall_sessions, validate_new};
pub use types::{
    ForgetReport, Layer, Memory, MemoryEntry, MemoryError, MemoryId, MemoryScope, NewMemory,
    Provenance, Recalled, RememberMode,
};

pub use core_bus_contract::{AgentId, SessionId};

/// Nazwy zdarzeń modułu (ładunki bez treści wpisów).
pub mod events {
    /// Zapamiętano wpis (`{ "memory", "layer", "trusted", "approved" }`).
    pub const REMEMBERED: &str = "memory.remembered";
    /// Wpis czeka na zatwierdzenie użytkownika.
    pub const PENDING_APPROVAL: &str = "memory.pending_approval";
    /// `recall` wykonany (Diagnostics: liczba zakresów, liczba wyników).
    pub const RECALLED: &str = "memory.recalled";
    /// Wpis zapomniany (raport kaskady).
    pub const FORGOTTEN: &str = "memory.forgotten";
}
