//! Kontrakt modułu `diagnostician` — Diagnosta (docs/modules/diagnostician/SPEC.md, PLAN §12.2,
//! ACCEPTANCE F8-01, F8-06).
//!
//! Przepływ: **sygnały** ([`Signal`]: stan modułów z rejestru, symptomy z dziennika Diagnostyka,
//! restarty i safe-mode watchdoga, budżety RAM/CPU, dysk) → **klasyfikacja** ([`classify`]:
//! katalog [`FailureKind`] ≥ 20 rodzajów, okno czasu, przyczyna szczegółowa wygrywa) →
//! **Propozycja zmiany** ([`plan`]: diff, uzasadnienie, ryzyko, plan cofnięcia) → **naprawa
//! cofalna** wg autonomii ([`RepairPolicy`]; każdy [`RepairStep`] ma [`RepairStep::inverse`],
//! nic nie jest usuwane) → **weryfikacja** (nieudana = automatyczne cofnięcie) → dziennik
//! append-only ([`JournalEntry`]) i raport „Zdrowie systemu” ([`HealthReport`]). Obszar Jądra
//! (moduły `core-*`, Broker, watchdog, updater, `kernel.*`, ścieżki Jądra) — **nigdy** przez
//! własny port Diagnosty: tylko prośba do Brokera ([`KernelApprovals`]). Diagnosta nie zmienia
//! własnej autonomii ani ustawień Ulepszacza, bezpieczeństwa, prywatności, budżetów i evals.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod catalog;
mod classify;
mod core;
mod events;
mod exec;
mod journal;
mod plan;
mod plan_rules;
mod policy;
mod ports;
mod report;
mod signal;
mod step;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use crate::core::DiagnosticianCore;
pub use catalog::{Area, FailureKind, Severity};
pub use classify::{ClassifierConfig, Detection, classify};
pub use events::{
    EVENT_FAILED, EVENT_INCIDENT, EVENT_NEEDS_HUMAN, EVENT_REPAIRED, EVENT_UNDONE, diag_event,
};
pub use journal::{Consent, JournalEntry, JournalEvent, RepairId, RepairRecord, RepairStatus};
pub use plan::{Proposal, Risk, key_segment, plan};
pub use policy::{RepairAutonomy, RepairPolicy, consent_for};
pub use ports::{
    DiagError, DiagHost, Diagnostician, KernelApprovals, KernelOutcome, NoBroker, RepairContext,
    RepairEnv, ScanOutcome, UserConsent,
};
pub use report::{
    HealthReport, HumanAction, IncidentRow, ModuleRow, Overall, ProposalCard, RepairRow, Usage,
    build_report,
};
pub use signal::{
    EVENT_SYMPTOM, ModuleCondition, Resource, Signal, Symptom, TimedSignal, WatchdogSignal,
    role_module,
};
pub use step::{
    DIAGNOSTICIAN_FORBIDDEN_PREFIXES, KERNEL_MODULES, RepairStep, is_forbidden_key, is_kernel_key,
    is_kernel_module,
};
