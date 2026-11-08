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
mod child;
mod guard;
pub mod hex;
pub mod ipc;
pub mod ipc_blocking;
mod judge;
mod policy;
mod proof;
mod protected;
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
    EVENT_APPROVAL_REQUESTED, EVENT_AUTONOMY_CHANGED, EVENT_CHAIN_STARTED, EVENT_CHILD_LOCK,
    EVENT_GRANT_PERSIST_FAILED, EVENT_GRANT_RESTORED, EVENT_GRANT_REVOKED, EVENT_KERNEL_BLOCK,
    EVENT_KEY_ROTATED, EVENT_KILL_SWITCH, EVENT_POLICY_CHANGED, EVENT_SESSION_TAINTED,
    EVENT_TOKEN_DENIED, EVENT_TOKEN_ISSUED, EVENT_TOKEN_REVOKED,
};
pub use approval::{
    ApprovalChallenge, ApprovalDecision, ApprovalId, ApprovalRequest, ApprovalStatus,
    ApprovalSubject, GrantInfo, PlanStepSummary, UNTIL_REVOKED,
};
pub use autonomy::{AutonomyEntry, AutonomyTable, AutonomyTarget, ModelLocation, ModelScope};
pub use capability::Capability;
pub use child::{
    CHILD_CEILING, ChildCeiling, ChildLockChange, ChildLockView, OPEN_UNTIL_CLOSE, PIN_BLOCK_MS,
    PIN_MAX_DIGITS, PIN_MAX_FAILURES, PIN_MIN_DIGITS, Pin,
};
pub use guard::KernelGuard;
pub use judge::{
    EVENT_JUDGE_DECIDED, EVENT_JUDGE_FUSE, EVENT_JUDGE_REQUESTED, EVENT_JUDGE_TIMEOUT,
    JUDGE_DRIVER_STALE_MS, JudgePolicy, JudgeRequest, JudgeResolution,
};
pub use policy::{
    HelloRequirement, KernelPolicy, PROTECTED_PROCESSES, PROTECTED_SERVICES, PROVIDER_APPS,
};
pub use proof::{InputSource, Nonce, PhysicalInputProof, broker_ui_only};
pub use protected::{is_protected_in, is_protected_scope};
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
