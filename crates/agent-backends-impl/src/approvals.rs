//! Rozstrzyganie próśb o uprawnienia: zdarzenie → `ApprovalSink` → pierwsza z decyzji
//! (kanał, `AgentBackend::approve`, limit czasu = odmowa, anulowanie = odmowa).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use agent_backends_contract::{
    AgentEvent, ApprovalDecision, ApprovalSink, BackendError, BridgeKind, PermissionKind,
    PermissionRequest, PermissionRequestId, TaskId,
};
use providers_contract::CancellationToken;
use serde_json::Value;
use tokio::sync::oneshot;

use crate::log::TaskLog;

type Pending = HashMap<PermissionRequestId, (TaskId, oneshot::Sender<ApprovalDecision>)>;

/// Prośba od mostu (bez identyfikatora — nadaje go hub).
#[derive(Debug, Clone)]
pub struct Ask {
    /// Most.
    pub bridge: BridgeKind,
    /// Rodzaj.
    pub kind: PermissionKind,
    /// Narzędzie.
    pub tool: String,
    /// Argumenty.
    pub input: Value,
    /// Uzasadnienie CLI.
    pub reason: Option<String>,
    /// Identyfikator wywołania w CLI.
    pub call_id: Option<String>,
}

/// Hub zatwierdzeń backendu.
pub struct Approvals {
    pending: Mutex<Pending>,
    counter: AtomicU64,
    sink: Arc<dyn ApprovalSink>,
    timeout: Duration,
}

impl Approvals {
    /// Nowy hub.
    pub fn new(sink: Arc<dyn ApprovalSink>, timeout: Duration) -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(0),
            sink,
            timeout,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Zadaje pytanie i czeka na decyzję.
    pub async fn ask(
        &self,
        log: &TaskLog,
        ask: Ask,
        cancel: &CancellationToken,
    ) -> ApprovalDecision {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let id = PermissionRequestId(format!("{}-perm-{n}", log.task()));
        let (tx, rx) = oneshot::channel();
        self.lock().insert(id.clone(), (log.task().clone(), tx));
        let request = PermissionRequest {
            id: id.clone(),
            task: log.task().clone(),
            bridge: ask.bridge,
            kind: ask.kind,
            tool: ask.tool,
            input: ask.input,
            reason: ask.reason,
            call_id: ask.call_id,
        };
        log.emit(AgentEvent::PermissionRequest {
            request: request.clone(),
        });
        let sink = self.sink.request(request);
        let (decision, timed_out) = tokio::select! {
            d = rx => (d.unwrap_or_else(|_| ApprovalDecision::deny("zadanie zakończone")), false),
            Some(d) = sink => (d, false),
            () = tokio::time::sleep(self.timeout) => (
                ApprovalDecision::deny("brak decyzji w wyznaczonym czasie — odmowa domyślna"),
                true,
            ),
            () = cancel.cancelled() => (ApprovalDecision::deny("zadanie anulowane"), false),
        };
        self.lock().remove(&id);
        log.emit(AgentEvent::PermissionResolved {
            id,
            decision: decision.clone(),
            timed_out,
        });
        decision
    }

    /// Decyzja z zewnątrz (`AgentBackend::approve`).
    pub fn resolve(
        &self,
        id: &PermissionRequestId,
        decision: ApprovalDecision,
    ) -> Result<(), BackendError> {
        let (_, tx) = self
            .lock()
            .remove(id)
            .ok_or_else(|| BackendError::UnknownPermissionRequest(id.0.clone()))?;
        tx.send(decision)
            .map_err(|_| BackendError::UnknownPermissionRequest(id.0.clone()))
    }

    /// Odrzuca wszystkie oczekujące prośby zadania (porzucenie nadawców = odmowa).
    pub fn drop_task(&self, task: &TaskId) {
        self.lock().retain(|_, (t, _)| t != task);
    }
}
