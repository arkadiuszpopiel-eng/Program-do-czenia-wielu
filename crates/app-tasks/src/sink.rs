//! Rejestr przebiegów mostów i kanał zatwierdzeń: prośby o uprawnienia mostu → kanał agentek
//! (Broker: decyzja wg poziomu autonomii, karta w Broker-UI, odmowa po czasie) z krokiem
//! „czeka na zatwierdzenie" w Replay.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use agent_backends_contract::{ApprovalDecision, ApprovalSink, PermissionKind, PermissionRequest};
use app_agents::Projection;
use app_api::dto::AgentRun;
use async_trait::async_trait;
use compliance_contract::PathEnv;
use core_bus_contract::SessionId;
use risk_classifier_contract::CommandOrigin;
use safety_broker_contract::{ApprovalId, ApprovalTicket};
use safety_broker_contract::{Capability, DeclaredFacts, Holder, PathScope};
use tools_common_contract::{BrokerGate, ToolCtx, ToolObserver, action_request};

use crate::bridge_view::BridgeProjector;
use crate::host::TaskHost;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Rejestr trwających przebiegów mostów (zadanie mostu → projektor).
#[derive(Default)]
pub struct BridgeRuns {
    runs: Mutex<BTreeMap<String, Arc<Mutex<BridgeProjector>>>>,
    host: OnceLock<Arc<dyn TaskHost>>,
}

impl BridgeRuns {
    /// Wiąże rdzeń (zapis Replay).
    pub fn bind(&self, host: Arc<dyn TaskHost>) {
        let _ = self.host.set(host);
    }

    /// Rejestruje przebieg.
    pub fn insert(&self, task: &str, projector: Arc<Mutex<BridgeProjector>>) {
        lock(&self.runs).insert(task.to_owned(), projector);
    }

    /// Usuwa przebieg.
    pub fn remove(&self, task: &str) {
        lock(&self.runs).remove(task);
    }

    fn with(
        &self,
        task: &str,
        f: impl FnOnce(&mut BridgeProjector) -> Projection,
    ) -> Option<(SessionId, AgentRun)> {
        let projector = lock(&self.runs).get(task).cloned()?;
        let mut guard = lock(&projector);
        let projection = f(&mut guard);
        let (session, run) = (guard.session().clone(), guard.run().clone());
        drop(guard);
        if let Some(host) = self.host.get() {
            host.project(&session, &run, projection);
        }
        Some((session, run))
    }
}

/// Karta „czeka na zatwierdzenie" w Replay mostu, gdy Broker prosi właściciela o decyzję.
struct SinkObserver {
    runs: Arc<BridgeRuns>,
    task: String,
    request: PermissionRequest,
}

impl ToolObserver for SinkObserver {
    fn approval_requested(&self, ticket: &ApprovalTicket) {
        let id = ticket.id.0.to_string();
        self.runs
            .with(&self.task, |p| p.permission(&self.request, Some(id)));
    }

    fn approval_resolved(&self, _id: ApprovalId, approved: bool) {
        self.runs.with(&self.task, |p| p.resolved(approved));
    }
}

/// Kanał zatwierdzeń mostów = kanał agentek (Broker przez `BrokerGate`).
pub struct BrokerSink {
    /// Broker (zwykle `TicketLog`).
    pub gate: BrokerGate,
    /// Przebiegi.
    pub runs: Arc<BridgeRuns>,
    /// Katalog kopii roboczych mostów (zakres zdolności).
    pub worktrees: String,
    /// Środowisko ścieżek właściciela.
    pub env: PathEnv,
    /// Limit czekania na decyzję.
    pub timeout: std::time::Duration,
}

impl BrokerSink {
    fn capability(&self, request: &PermissionRequest) -> Option<Capability> {
        let scope = PathScope::new(&self.worktrees, true, &self.env).ok()?;
        Some(match (request.kind, request.tool.as_str()) {
            (PermissionKind::Command, _) | (PermissionKind::Tool, "Bash") => {
                Capability::ShellExec(scope)
            }
            (PermissionKind::Tool, "Read" | "Glob" | "Grep" | "LS") => Capability::FsRead(scope),
            _ => Capability::FsWrite(scope),
        })
    }
}

#[async_trait]
impl ApprovalSink for BrokerSink {
    async fn request(&self, request: PermissionRequest) -> Option<ApprovalDecision> {
        let task = request.task.to_string();
        let deny = |m: &str| {
            Some(ApprovalDecision::Deny {
                message: m.to_owned(),
            })
        };
        let Some(capability) = self.capability(&request) else {
            return deny("Alfa: nieprawidłowy zakres katalogu mostu.");
        };
        let session = lock(&self.runs.runs)
            .get(&task)
            .map(|p| lock(p).session().clone());
        let Some(session) = session else {
            return deny("Alfa: nieznane zadanie mostu.");
        };
        let mut facts = DeclaredFacts::new(&format!("bridge.{}", request.tool));
        facts.untrusted_input_in_args = true;
        facts.command = request
            .input
            .get("command")
            .and_then(|c| c.as_str())
            .map(str::to_owned);
        let mut ctx = ToolCtx::new(Holder::agent(session.as_str(), "delta"));
        ctx.origin = CommandOrigin::Agent;
        ctx.approval_timeout = self.timeout;
        ctx.observer = Some(Arc::new(SinkObserver {
            runs: self.runs.clone(),
            task: task.clone(),
            request: request.clone(),
        }));
        let action = action_request(&ctx, capability, facts);
        let result = self.gate.authorize(action, &ctx).await;
        match result {
            Ok(auth) => {
                self.gate.release(&[auth]).await;
                Some(ApprovalDecision::Allow {
                    updated_input: None,
                })
            }
            Err(e) => deny(&format!("Alfa (Broker): {e}")),
        }
    }
}

/// Kanał zatwierdzeń bez Brokera: każda prośba mostu jest odrzucana (bez Brokera żadna akcja
/// nie jest dozwolona).
pub struct NoBrokerSink;

#[async_trait]
impl ApprovalSink for NoBrokerSink {
    async fn request(&self, _request: PermissionRequest) -> Option<ApprovalDecision> {
        Some(ApprovalDecision::Deny {
            message: "Alfa: Broker niepodłączony — prośba odrzucona.".into(),
        })
    }
}
