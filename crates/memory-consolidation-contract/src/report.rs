//! Raport przebiegu Strażniczki pamięci.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::Trigger;
use crate::policy::SkipReason;

/// Wynik jednego zakresu.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScopeRun {
    /// Klucz zakresu.
    pub scope: String,
    /// Wygaszone (retencja).
    pub expired: usize,
    /// Scalone duplikaty (operacje scalenia).
    pub merged: usize,
    /// Rozstrzygnięte sprzeczności.
    pub resolved: usize,
    /// Zgłoszone konflikty.
    pub conflicts: usize,
    /// Nowe wpisy z modelu (fakty, streszczenia, umiejętności).
    pub created: usize,
    /// Przetworzone epizody.
    pub consolidated: usize,
    /// Odrzucone propozycje modelu.
    pub rejected: usize,
}

/// Raport przebiegu (dziennik zmian: `run` w rekordach dziennika).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunReport {
    /// Identyfikator przebiegu.
    pub run: String,
    /// Wyzwalacz.
    pub trigger: Trigger,
    /// Start.
    pub started_at: DateTime<Utc>,
    /// Pominięty (nie wystartował).
    pub skipped: Option<SkipReason>,
    /// Przerwany między zakresami.
    pub interrupted: Option<SkipReason>,
    /// Zakresy.
    pub scopes: Vec<ScopeRun>,
    /// Wywołania modelu.
    pub llm_calls: usize,
    /// Budżet tła odmówił (model pominięty).
    pub budget_denied: bool,
    /// Propozycje awansu do globalnej (oczekujące na zgodę).
    pub proposals: usize,
    /// Błędy (bez treści wpisów).
    pub errors: Vec<String>,
}
