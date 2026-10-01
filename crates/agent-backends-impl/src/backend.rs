//! `BridgeBackend` — implementacja `AgentBackend` dla mostów Claude Code i Codex.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use agent_backends_contract::{
    AgentBackend, AgentEvent, AgentEventEnvelope, AgentEventStream, ApprovalDecision, ApprovalSink,
    BackendError, BridgeKind, EVENT_LAUNCH_REFUSED, EVENT_TASK_EVENT, PermissionRequestId,
    TaskHandle, TaskId, TaskSpec, Workspace,
};
use async_trait::async_trait;
use compliance_contract::Compliance;
use core_bus_contract::{Event, EventBus, EventKind, Level};
use mcp_contract::{BridgeMcpHost, BridgeScope};
use providers_contract::CancellationToken;
use tokio::sync::mpsc;

use crate::approvals::Approvals;
use crate::claude::{self, ClaudeApprovals};
use crate::codex;
use crate::config::BridgeConfig;
use crate::gate::Gate;
use crate::log::TaskLog;
use crate::process::{SystemTreeKiller, TreeKiller};
use crate::runner::RunCtx;

/// Zależności backendu (porty innych modułów).
pub struct BackendDeps {
    /// Rejestr zgodności (`route_allowed`, przypięcie wersji z rejestru).
    pub compliance: Arc<dyn Compliance>,
    /// Izolowany katalog roboczy.
    pub workspace: Arc<dyn Workspace>,
    /// Host serwera MCP Alfy (narzędzie `approve` dla Claude Code).
    pub mcp: Arc<dyn BridgeMcpHost>,
    /// Kanał zatwierdzeń.
    pub sink: Arc<dyn ApprovalSink>,
}

struct Entry {
    log: Arc<TaskLog>,
    cancel: CancellationToken,
    steer: mpsc::UnboundedSender<String>,
}

/// Backend mostów CLI.
pub struct BridgeBackend {
    config: Arc<BridgeConfig>,
    gate: Gate,
    deps: BackendDeps,
    approvals: Arc<Approvals>,
    killer: Arc<dyn TreeKiller>,
    tasks: Mutex<HashMap<TaskId, Entry>>,
    spawned: AtomicU64,
    counter: AtomicU64,
    forward: Option<mpsc::UnboundedSender<AgentEventEnvelope>>,
    bus: Option<Arc<dyn EventBus>>,
}

fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(text.as_bytes())
    }
    #[cfg(not(unix))]
    {
        // %LOCALAPPDATA% dziedziczy ACL tylko dla użytkownika.
        std::fs::write(path, text)
    }
}

impl BridgeBackend {
    /// Nowy backend.
    pub fn new(config: BridgeConfig, deps: BackendDeps) -> Self {
        let config = Arc::new(config);
        Self {
            gate: Gate::new(deps.compliance.clone(), config.clone()),
            approvals: Arc::new(Approvals::new(deps.sink.clone(), config.approval_timeout)),
            config,
            deps,
            killer: Arc::new(SystemTreeKiller),
            tasks: Mutex::new(HashMap::new()),
            spawned: AtomicU64::new(0),
            counter: AtomicU64::new(0),
            forward: None,
            bus: None,
        }
    }

    /// Podmienia zabójcę drzew procesów (np. Job Object z `platform-windows-impl`).
    #[must_use]
    pub fn with_tree_killer(mut self, killer: Arc<dyn TreeKiller>) -> Self {
        self.killer = killer;
        self
    }

    /// Publikuje każde zdarzenie zadania na magistralę (`agent.bridge.event`, ładunek z
    /// `unverified_by_alfa = true`). Wymaga działającego środowiska tokio.
    #[must_use]
    pub fn with_bus(mut self, bus: Arc<dyn EventBus>) -> Self {
        self.bus = Some(bus.clone());
        let (tx, mut rx) = mpsc::unbounded_channel::<AgentEventEnvelope>();
        tokio::spawn(async move {
            while let Some(env) = rx.recv().await {
                let payload = serde_json::to_value(&env).unwrap_or_default();
                let event = Event::new(
                    EventKind::Custom(EVENT_TASK_EVENT.to_owned()),
                    Level::Info,
                    payload,
                );
                let _ = bus.publish(event).await;
            }
        });
        self.forward = Some(tx);
        self
    }

    /// Liczba procesów uruchomionych przez backend (zadania CLI + `--version`).
    pub fn processes_spawned(&self) -> u64 {
        self.spawned.load(Ordering::SeqCst) + self.gate.probes()
    }

    fn tasks(&self) -> MutexGuard<'_, HashMap<TaskId, Entry>> {
        self.tasks.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn with_entry<T>(&self, task: &TaskId, f: impl FnOnce(&Entry) -> T) -> Result<T, BackendError> {
        self.tasks()
            .get(task)
            .map(f)
            .ok_or_else(|| BackendError::UnknownTask(task.0.clone()))
    }

    fn mcp_config_path(
        &self,
        task: &TaskId,
        doc: &serde_json::Value,
    ) -> Result<PathBuf, BackendError> {
        let dir = self.config.runtime_dir.join(&task.0);
        std::fs::create_dir_all(&dir).map_err(|e| BackendError::Mcp(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| BackendError::Mcp(e.to_string()))?;
        }
        let path = dir.join("mcp-config.json");
        write_private(&path, &doc.to_string()).map_err(|e| BackendError::Mcp(e.to_string()))?;
        Ok(path)
    }
}

#[async_trait]
impl AgentBackend for BridgeBackend {
    async fn submit_task(&self, spec: TaskSpec) -> Result<TaskHandle, BackendError> {
        let admitted = match self.gate.check(&spec).await {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(most = %spec.bridge, blad = %e, "odmowa uruchomienia mostu");
                if let Some(bus) = &self.bus {
                    let payload = serde_json::json!({
                        "bridge": spec.bridge, "origin": spec.origin, "error": e,
                        "alfa_session": spec.alfa_session,
                    });
                    let kind = EventKind::Custom(EVENT_LAUNCH_REFUSED.to_owned());
                    let _ = bus.publish(Event::new(kind, Level::Warn, payload)).await;
                }
                return Err(e);
            }
        };
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let short = uuid::Uuid::new_v4().simple().to_string();
        let task = TaskId(format!("task-{n}-{}", &short[..8]));
        let prepared = match &spec.session {
            Some(session) => self.deps.workspace.reuse(session).await?,
            None => self.deps.workspace.prepare(&task, &spec.workdir).await?,
        };
        let log = TaskLog::new(task.clone(), self.forward.clone());
        let cancel = CancellationToken::new();
        let (steer_tx, steer_rx) = mpsc::unbounded_channel();
        let ctx = RunCtx {
            log: log.clone(),
            approvals: self.approvals.clone(),
            cancel: cancel.clone(),
            steer: steer_rx,
            killer: self.killer.clone(),
            config: self.config.clone(),
            workdir: prepared.path.clone(),
            budget: spec.budget.clone(),
            bridge: spec.bridge,
        };
        let registration = if spec.bridge == BridgeKind::ClaudeCode {
            let router = Arc::new(ClaudeApprovals {
                approvals: self.approvals.clone(),
                log: log.clone(),
                cancel: cancel.clone(),
            });
            let reg = match self
                .deps
                .mcp
                .register(BridgeScope::windows_v0(task.0.clone()), Some(router))
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = self.deps.workspace.release(&prepared, false).await;
                    return Err(BackendError::Mcp(e.to_string()));
                }
            };
            Some(reg)
        } else {
            None
        };
        let claude_args = match &registration {
            Some(reg) => match self.mcp_config_path(&task, &reg.launch.to_mcp_config()) {
                Ok(path) => Some(claude::args(
                    &spec,
                    &path,
                    self.config.claude_partial_messages,
                )),
                Err(e) => {
                    let _ = self.deps.mcp.revoke(&reg.id).await;
                    let _ = self.deps.workspace.release(&prepared, false).await;
                    return Err(e);
                }
            },
            None => None,
        };
        log.emit(AgentEvent::Started {
            bridge: spec.bridge,
            cli_version: admitted.version,
            workdir: prepared.path.clone(),
        });
        self.tasks().insert(
            task.clone(),
            Entry {
                log: log.clone(),
                cancel,
                steer: steer_tx,
            },
        );
        self.spawned.fetch_add(1, Ordering::SeqCst);
        let (mcp, approvals, runtime) = (
            self.deps.mcp.clone(),
            self.approvals.clone(),
            self.config.runtime_dir.join(&task.0),
        );
        let task_id = task.clone();
        let program = admitted.program;
        tokio::spawn(async move {
            match claude_args {
                Some(args) => claude::run(ctx, program, args, spec.prompt).await,
                None => codex::run(ctx, program, spec.prompt, spec.session, spec.model).await,
            }
            if let Some(reg) = registration {
                let _ = mcp.revoke(&reg.id).await;
                let _ = std::fs::remove_dir_all(&runtime);
            }
            approvals.drop_task(&task_id);
        });
        Ok(TaskHandle {
            task,
            bridge: spec.bridge,
            workdir: prepared.path,
        })
    }

    fn events(&self, task: &TaskId) -> Result<AgentEventStream, BackendError> {
        self.with_entry(task, |e| e.log.stream())
    }

    async fn approve(
        &self,
        request: &PermissionRequestId,
        decision: ApprovalDecision,
    ) -> Result<(), BackendError> {
        self.approvals.resolve(request, decision)
    }

    async fn steer(&self, task: &TaskId, message: String) -> Result<(), BackendError> {
        let (finished, tx) = self.with_entry(task, |e| (e.log.is_finished(), e.steer.clone()))?;
        if finished {
            return Err(BackendError::TaskFinished);
        }
        tx.send(message)
            .map_err(|_| BackendError::SteeringUnavailable("zadanie kończy się".into()))
    }

    async fn cancel(&self, task: &TaskId) -> Result<(), BackendError> {
        let (finished, cancel) =
            self.with_entry(task, |e| (e.log.is_finished(), e.cancel.clone()))?;
        if finished {
            return Err(BackendError::TaskFinished);
        }
        cancel.cancel();
        self.approvals.drop_task(task);
        Ok(())
    }
}
