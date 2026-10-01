//! Kontrakt Brokera — Safety Kernel (docs/modules/safety-broker/SPEC.md, PLAN §8, ADR 3, ADR 15).
//!
//! Zawiera: zdolności z zakresami i atenuacją ([`Capability`], potomek ≤ rodzic), token
//! zdolności z kanonicznym formatem przewodowym ([`CapToken`]), poziomy autonomii per
//! sesja/agentka ([`AutonomyTable`]), dowód fizycznego wejścia konstruowalny tylko przez
//! Broker-UI ([`PhysicalInputProof`]), prośby i decyzje, polityki Jądra ([`KernelPolicy`]),
//! strażnika twardych reguł ([`KernelGuard`], [`check_command`]), traity [`Broker`],
//! [`ApprovalChannel`], [`AnchorStore`] oraz protokół IPC ([`ipc`]). Czysta logika reguł jest
//! tutaj, żeby `-impl` i `-fake` nie mogły się rozjechać.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod action;
mod api;
mod approval;
mod autonomy;
mod capability;
mod guard;
pub mod hex;
pub mod ipc;
mod policy;
mod proof;
mod scope;
mod shell_guard;
mod token;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use action::{
    ActionRequest, ApprovalTicket, AttenuateRequest, AutonomyChangeRequest, BrokerError,
    BrokerMetrics, ChangeOrigin, Decision, DeclaredFacts, DenyReason, PlanDecision, PlanRequest,
    PlanStep, SessionSecurity, TaintSource,
};
pub use api::{
    AnchorStore, ApprovalChannel, Broker, ChainAnchor, EVENT_APPROVAL_DECIDED,
    EVENT_APPROVAL_REQUESTED, EVENT_AUTONOMY_CHANGED, EVENT_CHAIN_STARTED, EVENT_KERNEL_BLOCK,
    EVENT_KEY_ROTATED, EVENT_KILL_SWITCH, EVENT_POLICY_CHANGED, EVENT_SESSION_TAINTED,
    EVENT_TOKEN_DENIED, EVENT_TOKEN_ISSUED, EVENT_TOKEN_REVOKED,
};
pub use approval::{
    ApprovalChallenge, ApprovalDecision, ApprovalId, ApprovalRequest, ApprovalStatus,
    ApprovalSubject, PlanStepSummary,
};
pub use autonomy::{AutonomyEntry, AutonomyTable, AutonomyTarget};
pub use capability::Capability;
pub use guard::KernelGuard;
pub use policy::{HelloRequirement, KernelPolicy, PROTECTED_PROCESSES, PROTECTED_SERVICES};
pub use proof::{InputSource, Nonce, PhysicalInputProof, broker_ui_only};
pub use scope::{
    AdminOp, AppSelector, HostPattern, PathScope, ScopeError, SecretId, ServiceAction, render_path,
};
pub use shell_guard::{ShellContext, check_command};
pub use token::{BootId, CapToken, Holder, MAC_LEN, MAX_TOKEN_BYTES, TokenId, WireError};

pub use risk_classifier_contract::{AutonomyLevel, CommandOrigin, KernelRule};

use core_bus_contract::EventKind;

/// Rodzaj zdarzenia jako `EventKind` (zdarzenia Brokera mają poziom `Audit`).
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}
