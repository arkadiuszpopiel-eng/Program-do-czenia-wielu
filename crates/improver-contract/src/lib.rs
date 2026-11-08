//! Kontrakt modułu `improver` — Ulepszacz (docs/modules/improver/SPEC.md, PLAN §12.1, §12.4,
//! THREAT_MODEL S20, ACCEPTANCE F8-02 … F8-04).
//!
//! Potok: **obserwacja** (R0 wg zadania: metryki i wyniki evali → reguły retrospektywy
//! [`rules`] i niezaufane porty [`Proposer`]) → **strażnik** ([`assess`]: zamknięta lista
//! kluczy [`IMPROVABLE`]; `kernel.*`, prywatność, budżety, uprawnienia, autonomia, egress,
//! progi i zestawy `evals`, własne ustawienia Ulepszacza — zawsze odrzucone; brak portu plików;
//! kod = tylko szkic zgłoszenia R3) → **piaskownica** (podział `test`, przed/po) i **holdout**
//! (bramka Jądra [`evals_contract::EvalGate`], wynik zbiorczy, N ≥ 5) → **wdrożenie** przez
//! `core-config` z `Origin::Improver` (automatycznie wyłącznie pierścień R0 i zmiany zawężające
//! albo bezpieczne; resztę zatwierdza użytkownik — R1/R2 z podpisem) → **nadzór**: regresja
//! metryk = automatyczny rollback i wychładzanie klucza. Ulepszacz nie ma API zmiany własnej
//! polityki ani autonomii ([`ImproverPolicy`] tylko do odczytu).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod core;
mod events;
mod guard;
mod pipeline;
mod policy;
mod ports;
mod proposal;
mod ring;
pub mod rules;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use crate::core::ImproverCore;
pub use events::{
    EVENT_BLOCKED, EVENT_DEPLOYED, EVENT_EVALUATED, EVENT_PROPOSED, EVENT_REJECTED,
    EVENT_ROLLED_BACK, improver_event,
};
pub use guard::{Assessment, ChangeTarget, Violation, assess, check_key};
pub use pipeline::regression;
pub use policy::{ImproverPolicy, WatchedMetric};
pub use ports::{ApprovalVerifier, Improver, ImproverError, ImproverHost, Proposer};
pub use proposal::{
    BlockedAttempt, CandidateSet, IssueDraft, MetricsSnapshot, Observation, PlannedChange,
    Proposal, ProposalId, RunConditions, Stage, StageNote, UserApproval,
};
pub use ring::{
    FORBIDDEN_PREFIXES, FORBIDDEN_SEGMENTS, IMPROVABLE, KeyRule, NarrowDir, Ring, SafetyClass,
    ValueKind, pattern_matches, rule_for,
};
