//! Kontrakt konsolidacji pamięci — **Strażniczka pamięci** (PLAN §10 „nocna konsolidacja”, §9.2;
//! docs/modules/memory-consolidation/SPEC.md).
//!
//! Przebieg ([`Guardian::run`]) działa na [`memory_contract::MemoryService`] jako
//! [`memory_contract::Accessor::Guardian`]:
//! 1. polityka ([`may_start`]): nie na baterii, nie w trybie gry/pełnego ekranu (każdy wyzwalacz),
//!    w oknie nocnym i po bezczynności (harmonogram); sprawdzana też między zakresami;
//! 2. reguły deterministyczne ([`rules`]): retencja (TTL, stare przetworzone epizody — nieodwracalne),
//!    deduplikacja (scalenie z przywracaniem), sprzeczności tematów (nowsza wersja albo konflikt);
//! 3. model językowy ([`Consolidator`], produkcyjnie `ModelProvider` lokalny) z budżetem tła
//!    ([`BackgroundBudget`] → `cost-meter`): tylko epizody zaufane, sesje prywatne tylko modelem
//!    lokalnym, fakty wg `auto_extract` (domyślnie oczekujące), umiejętności zawsze oczekujące;
//! 4. propozycje awansu do pamięci globalnej (oczekujące — zgoda użytkownika).
//!
//! Każda zmiana trafia do dziennika pamięci z identyfikatorem przebiegu — [`undo_run`] cofa całość.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod config;
mod guardian;
mod policy;
mod ports;
mod promote;
mod report;
pub mod rules;

pub use config::{AutoExtract, ConsolidationConfig, Trigger, in_window};
pub use guardian::{Guardian, GuardianPorts};
pub use policy::{SkipReason, may_start};
pub use ports::{
    BackgroundBudget, BudgetVerdict, ConsolidationBatch, ConsolidationError, Consolidator,
    ConsolidatorModel, ConsolidatorOutput, EpisodeView, FactView, HostConditions, HostState,
    LlmUsage, ProposedFact, ProposedSkill, ProposedSummary,
};
pub use promote::undo_run;
pub use report::{RunReport, ScopeRun};

/// Nazwy zdarzeń (z `memory-contract`): start, koniec, pominięcie przebiegu.
pub use memory_contract::events::{
    CONSOLIDATION_FINISHED, CONSOLIDATION_SKIPPED, CONSOLIDATION_STARTED,
};
