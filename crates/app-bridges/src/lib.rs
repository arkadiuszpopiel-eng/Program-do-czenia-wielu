//! Mosty CLI i MCP w aplikacji (kategoria `app-*`, wydzielona z `app-core` — limit rozmiaru
//! crate'a):
//! - [`BridgesApp`] — karty zgodności tras (`compliance`: zielona/szara/zabroniona, nieświeży
//!   wpis, wersja wykryta vs przypięta, regulamin, data weryfikacji), wyłącznik trasy, zgoda na
//!   harmonogram per trasa, „Zaloguj w terminalu" (logowanie wyłącznie przez użytkownika);
//! - [`BridgeHandle`] — `AgentBackend` nad `agent-backends-impl` przebudowywanym po zmianie
//!   ustawień (zadania trafiają do instancji, która je przyjęła);
//! - [`LazyMcpHost`] — serwer MCP Alfy v0 + v1 ([`mcp_v1`]: UIA, zrzuty, rejestr przez Brokera)
//!   uruchamiany na żądanie mostu;
//! - [`parse_delegation`] — „Delta, zleć to Claude Code" w czacie.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod app;
pub mod cards;
mod delegate;
mod mcp;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use agent_backends_contract::{
    AgentBackend, AgentEventStream, ApprovalDecision, BackendError, PermissionRequestId,
    TaskHandle, TaskId, TaskSpec,
};
use async_trait::async_trait;

pub use app::{BridgesApp, BridgesParts, DETECT_TTL, MAX_SCHEDULE_PER_DAY};
pub use delegate::parse_delegation;
pub use mcp::{LazyMcpHost, mcp_v1, proxy_program};

/// Manifesty modułów składanych przez ten crate (rejestr `core-registry` w `app-core`).
pub const MODULES: &[(&str, &str)] = &[
    ("agent-backends", agent_backends_impl::MODULE_TOML),
    ("mcp", mcp_impl::MODULE_TOML),
];

/// Backend mostów dla wykonawczyni zadań.
pub struct BridgeHandle {
    app: Arc<BridgesApp>,
    owners: Mutex<BTreeMap<TaskId, Arc<dyn AgentBackend>>>,
}

impl BridgeHandle {
    /// Uchwyt nad mostami aplikacji.
    pub fn new(app: Arc<BridgesApp>) -> Self {
        Self {
            app,
            owners: Mutex::new(BTreeMap::new()),
        }
    }

    fn owners(&self) -> MutexGuard<'_, BTreeMap<TaskId, Arc<dyn AgentBackend>>> {
        self.owners.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn owner(&self, task: &TaskId) -> Result<Arc<dyn AgentBackend>, BackendError> {
        self.owners()
            .get(task)
            .cloned()
            .ok_or_else(|| BackendError::UnknownTask(task.0.clone()))
    }
}

#[async_trait]
impl AgentBackend for BridgeHandle {
    async fn submit_task(&self, spec: TaskSpec) -> Result<TaskHandle, BackendError> {
        let backend = self.app.backend().await;
        let handle = backend.submit_task(spec).await?;
        self.owners().insert(handle.task.clone(), backend);
        Ok(handle)
    }

    fn events(&self, task: &TaskId) -> Result<AgentEventStream, BackendError> {
        self.owner(task)?.events(task)
    }

    async fn approve(
        &self,
        request: &PermissionRequestId,
        decision: ApprovalDecision,
    ) -> Result<(), BackendError> {
        let backends: Vec<Arc<dyn AgentBackend>> = self.owners().values().cloned().collect();
        let mut last = Err(BackendError::InvalidSpec(format!(
            "nieznana prośba {request}"
        )));
        for b in backends {
            last = b.approve(request, decision.clone()).await;
            if last.is_ok() {
                break;
            }
        }
        last
    }

    async fn steer(&self, task: &TaskId, message: String) -> Result<(), BackendError> {
        self.owner(task)?.steer(task, message).await
    }

    async fn cancel(&self, task: &TaskId) -> Result<(), BackendError> {
        let result = self.owner(task)?.cancel(task).await;
        self.owners().remove(task);
        result
    }
}
