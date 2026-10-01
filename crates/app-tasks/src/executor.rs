//! Wykonawczyni zadań aplikacji: agentka (`agent-runtime` v1 z hakiem granicy kroku), most CLI
//! (`agent-backends`, pochodzenie zadania decyduje o zgodzie) albo usługa systemowa.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use scheduler_contract::{Dispatch, ExecutorKind, StepGate, TaskExecutor, WorkerResult};

use crate::exec_agent::{TaskRuntimes, fail};
use crate::host::ExecDeps;

/// Wykonawczyni zadań.
pub struct AppExecutor {
    deps: ExecDeps,
    runtimes: TaskRuntimes,
    bridges_denied: Arc<AtomicBool>,
}

impl AppExecutor {
    /// Nowa wykonawczyni; `bridges_denied` — polityka Marszałka (mosty zabronione regułą).
    pub fn new(deps: ExecDeps, bridges_denied: Arc<AtomicBool>) -> Self {
        Self {
            deps,
            runtimes: TaskRuntimes::default(),
            bridges_denied,
        }
    }

    /// Runtime trwających zadań agentek (diagnostyka).
    pub fn runtimes(&self) -> &TaskRuntimes {
        &self.runtimes
    }
}

#[async_trait]
impl TaskExecutor for AppExecutor {
    async fn execute(&self, dispatch: Dispatch, gate: Arc<dyn StepGate>) -> WorkerResult {
        match dispatch.spec.executor.clone() {
            ExecutorKind::Agent => {
                crate::exec_agent::run(&self.deps, &self.runtimes, dispatch, gate).await
            }
            ExecutorKind::Bridge(_) if self.bridges_denied.load(Ordering::SeqCst) => fail(
                "Reguła Marszałka zabrania mostów CLI (Ustawienia → Agentki → Reguły).",
                false,
            ),
            ExecutorKind::Bridge(kind) => {
                crate::exec_bridge::run(&self.deps, kind, dispatch, gate).await
            }
            ExecutorKind::Service(name) => fail(format!("nieznana usługa „{name}”"), false),
        }
    }
}
