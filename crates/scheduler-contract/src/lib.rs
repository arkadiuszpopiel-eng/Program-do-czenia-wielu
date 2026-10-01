//! Kontrakt pełnego `scheduler` (docs/modules/scheduler/SPEC.md, PLAN §9.3, §9.6, §16.2 F5).
//!
//! Deterministyczny szeregowacz zadań agentek — **nadzbiór `scheduler-lite`**: ta sama tablica
//! blokad zasobów wyłącznych (głośnik, mikrofon, ekran+wejście, pliki), te same typy
//! (`Resource`, `Holder`, `Priority`, `Lease`), a do tego:
//! - zadania jako **DAG** z warunkami (sukces/porażka/dowolnie/wynik pośredni) i delegacją
//!   (drzewo rodzic → podzadania; anulowanie poddrzewa),
//! - przydział do agentek wg ról i dostępności ([`Roster`]), limity równoległości,
//! - priorytety (mowa użytkownika > zadania użytkownika > agentek > tła) z wywłaszczaniem
//!   wyłącznie w punktach atomowych,
//! - okna czasowe (nie wcześniej niż, termin, tylko w bezczynności, nie w trybie gry),
//! - budżety (kroki, czas, koszt; budżet tła z `cost-meter`), ponowienia z odstępem,
//! - steering (≤ 1 krok atomowy), trwałość stanu (restart = wznowienie), zdarzenia
//!   `scheduler.task.*`.
//!
//! Reguły decyzyjne są w kontrakcie ([`SchedCore`]) — `-impl` i `-fake` różnią się tylko
//! otoczeniem ([`SchedHost`]), więc nie mogą się rozjechać.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod engine;
mod error;
mod events;
mod host;
mod ids;
mod roster;
mod spec;
mod state;
mod steer;
mod validate;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use std::sync::Arc;

use async_trait::async_trait;

pub use engine::{SNAPSHOT_VERSION, SchedCore, SchedEffect, Snapshot};
pub use error::TaskError;
pub use events::{
    ALL_EVENTS, EVENT_ABORTED, EVENT_BLOCKED, EVENT_BUDGET_WARNING, EVENT_DISPATCHED,
    EVENT_FINISHED, EVENT_KILL_SWITCH, EVENT_LOOP, EVENT_PAUSED, EVENT_RESTORED, EVENT_RESUMED,
    EVENT_RETRY, EVENT_STEER_UNCONSUMED, EVENT_STEERED, EVENT_STEP, EVENT_SUBMITTED, EVENT_YIELDED,
    event_kind, short_title,
};
pub use host::{LiteBridge, MemSnapshotStore, SchedHost, SnapshotStore};
pub use ids::{DispatchId, MAX_TASK_ID_LEN, TaskId};
pub use roster::{AgentSlot, Roster, SystemConditions};
pub use spec::{
    Assignee, DepCondition, Dependency, ExecutorKind, RetryPolicy, TaskBudget, TaskClass,
    TaskOrigin, TaskSpec, TimeWindow,
};
pub use state::{
    BlockReason, BudgetKind, CancelCause, ExpiryReason, TaskOutput, TaskState, TaskView,
    Termination,
};
pub use steer::{
    Dispatch, Steer, SteerEnvelope, SteerVia, StepDirective, StepReport, StopReason, WorkerResult,
    YieldReason,
};
pub use validate::{
    DEFAULT_DEADLINE_MS, LOOP_REPEATS, MAX_ACTIVE_TASKS, MAX_ATTEMPTS, MAX_BACKOFF_MS, MAX_DEPS,
    MAX_OUTPUT_BYTES, MAX_PAYLOAD_BYTES, MAX_PENDING_STEERS, MAX_RESOURCES, MAX_STEPS,
    MAX_TASKS_PER_SUBMIT, MAX_WALL_MS, RETENTION_MS, STOP_GRACE_MS, effective_deadline,
    validate_spec,
};

pub use agent_backends_contract::{BridgeKind, LaunchOrigin};
pub use scheduler_lite_contract::{
    Holder, Lease, LeaseInfo, LeaseRequest, OnTimeout, PreemptReason, Priority, QueuedRequest,
    Resource, ResourcePolicy, SchedError, SchedulerLite,
};

/// Pełny scheduler. Jako nadzbiór `scheduler-lite` obsługuje też dzierżawy mowy (`acquire`,
/// `preempt`, `handoff`) na tej samej tablicy blokad; `kill_all` zatrzymuje także zadania.
#[async_trait]
pub trait Scheduler: SchedulerLite {
    /// Zgłasza zadania (całość albo nic: walidacja, unikalność, zależności, brak cykli).
    fn submit(&self, tasks: Vec<TaskSpec>) -> Result<Vec<TaskId>, TaskError>;

    /// Anuluje zadanie i całe jego poddrzewo delegacji; zwraca objęte zadania.
    fn cancel(&self, task: &TaskId, reason: &str) -> Result<Vec<TaskId>, TaskError>;

    /// Steering: treść trafia do wykonawczyni w najbliższym punkcie atomowym (≤ 1 krok);
    /// pauza/wznowienie/anulowanie działają od razu. Zwraca numer wiadomości (0 dla operacji).
    fn steer(&self, task: &TaskId, steer: Steer) -> Result<u64, TaskError>;

    /// Wstrzymuje zadanie (w toku: w najbliższym punkcie atomowym, zasoby oddane).
    fn pause(&self, task: &TaskId) -> Result<(), TaskError>;

    /// Wznawia wstrzymane zadanie.
    fn resume(&self, task: &TaskId) -> Result<(), TaskError>;

    /// Widok zadania.
    fn task(&self, task: &TaskId) -> Option<TaskView>;

    /// Wszystkie zadania (kolejność zgłoszeń).
    fn tasks(&self) -> Vec<TaskView>;

    /// Nowa obsada (zmiana obsady w `personas`).
    fn set_roster(&self, roster: Roster);

    /// Nowe warunki systemowe (bezczynność, tryb gry).
    fn set_conditions(&self, conditions: SystemConditions);

    /// Czeka na zakończenie zadania.
    async fn wait(&self, task: &TaskId) -> Result<Termination, TaskError>;
}

/// Punkt atomowy widziany przez wykonawczynię.
pub trait StepGate: Send + Sync {
    /// Ukończyłam krok i chcę zacząć następny: co dalej? (`Continue` niesie steering.)
    fn boundary(&self, report: StepReport) -> StepDirective;

    /// Delegacja: podzadania tego zadania (pochodzenie i taint dziedziczone).
    fn spawn(&self, tasks: Vec<TaskSpec>) -> Result<Vec<TaskId>, TaskError>;
}

/// Port wykonawczyni (adaptery w `app-*`: `agent-runtime`, `agent-backends`, usługi).
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// Wykonuje zadanie, wołając `gate.boundary` między krokami atomowymi.
    async fn execute(&self, dispatch: Dispatch, gate: Arc<dyn StepGate>) -> WorkerResult;
}
