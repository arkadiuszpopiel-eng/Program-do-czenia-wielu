//! Kontrakt modułu `evals` — harness ewaluacji (docs/modules/evals/SPEC.md, PLAN §4.4, §12.4,
//! ACCEPTANCE §1 i F8-03).
//!
//! Zestaw = manifest ([`SuiteManifest`]) z sumami SHA-256 wszystkich plików, statusem
//! (`proposed` → `frozen` po akceptacji człowieka) i progami ([`Threshold`]); przypadki
//! ([`EvalCase`]) mają podział [`Split`]: `dev`, `test` albo `holdout`. Zmiana pliku zestawu
//! zamrożonego to błąd ([`EvalError::IntegrityViolation`]). Holdout żyje poza gitem i poza
//! katalogiem ([`SuiteCatalog`] zwraca [`EvalError::HoldoutSealed`]); jedyną drogą do niego jest
//! bramka ([`EvalGate`]) zwracająca **wynik zbiorczy** ([`GateVerdict`]) — bez identyfikatorów
//! i treści przypadków, z zaokrągleniem i budżetem zapytań (ochrona przed Goodhartem, §12.4).
//! Warianty uruchamia port [`CandidateRunner`] dostarczany przez Jądro (kompozycja `app-*`),
//! nigdy przez Ulepszacza — dzięki temu kod Ulepszacza nie widzi wejść holdoutu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod aggregate;
mod case;
mod catalog;
mod clock;
mod compare;
mod error;
mod events;
mod formats;
mod gate;
mod integrity;
mod legacy;
mod manifest;
mod report;
mod run;
mod stats;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use aggregate::{
    Aggregate, ClassAggregate, PASS_RATE, RUNNER_ERROR, ThresholdResult, ThresholdStatus,
    aggregate, check_thresholds, per_case_means,
};
pub use case::{CaseOutcome, EvalCase, validate_cases};
pub use catalog::{SuiteCatalog, SuiteInfo};
pub use clock::{Clock, ManualClock, SystemClock};
pub use compare::{CompareVerdict, Direction, VariantComparison, compare};
pub use error::EvalError;
pub use events::{
    EVENT_GATE_DECIDED, EVENT_INTEGRITY_FAILED, EVENT_RUN_COMPLETED, evals_event, event_kind,
};
pub use formats::parse_cases;
pub use gate::{
    CandidateRunner, EvalGate, GateDecision, GatePolicy, GateRequest, GateStage, GateVerdict,
    MIN_GATE_REPEATS, Variant, VariantSummary,
};
pub use integrity::{FileMismatch, IntegrityReport, is_sha256_hex, sha256_hex, verify_files};
pub use legacy::LegacyManifest;
pub use manifest::{
    CaseFormat, CaseSource, Comparison, MANIFEST_SCHEMA_VERSION, Split, SuiteId, SuiteManifest,
    SuiteStatus, Threshold, ThresholdRule, validate_rel_path,
};
pub use report::{EvalReport, build_report};
pub use run::{QueryBudget, decide, evaluate_cases, run_variant};
pub use stats::{
    BootstrapConfig, Interval, SplitMix64, bootstrap_mean, bootstrap_paired_delta, mean,
};
