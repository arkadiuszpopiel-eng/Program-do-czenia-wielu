//! Kontrakt `AgentBackend` (docs/modules/agent-backends/SPEC.md, PLAN §5.1, §5.2 klasa C, §8.5,
//! ADR 0005): zadanie → strumień zdarzeń, zatwierdzenia, sterowanie, anulowanie, wznawianie.
//!
//! Mosty CLI (Claude Code, Codex) to „opaque worker”: oficjalne, niezmodyfikowane CLI, do którego
//! loguje się **użytkownik**; Alfa nigdy nie czyta ani nie przechowuje jego tokenów. Czyste reguły
//! zgodności (pochodzenie uruchomienia, przypięte wersje, środowisko procesu) są w [`policy`],
//! żeby `-impl` i `-fake` nie mogły się rozjechać.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod approval;
mod backend;
mod error;
mod event;
pub mod policy;
mod task;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use approval::{
    ApprovalDecision, ApprovalSink, DEFAULT_APPROVAL_TIMEOUT_MS, PermissionKind, PermissionRequest,
    PermissionRequestId,
};
pub use backend::{AgentBackend, AgentEventStream, PreparedWorkdir, WorkdirKind, Workspace};
pub use error::{BackendError, LaunchRefusal};
pub use event::{
    AgentEvent, AgentEventEnvelope, FileChangeKind, MAX_PREVIEW_CHARS, OutputFormat, PlanItem,
    TaskResult, preview,
};
pub use policy::{CliPin, LaunchPolicy, ScheduleConsent, check_origin, check_version, child_env};
pub use task::{
    BridgeKind, LaunchOrigin, SessionRef, TaskBudget, TaskHandle, TaskId, TaskSpec, WorkdirMode,
    WorkdirSpec,
};

/// Zdarzenie magistrali: każde zdarzenie zadania (ładunek = `AgentEventEnvelope`).
pub const EVENT_TASK_EVENT: &str = "agent.bridge.event";
/// Zdarzenie magistrali: odmowa uruchomienia mostu (pochodzenie, trasa, wersja).
pub const EVENT_LAUNCH_REFUSED: &str = "agent.bridge.refused";
