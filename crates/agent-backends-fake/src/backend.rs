//! `FakeAgentBackend`: scenariusze w pamięci na tokio (krótkie, deterministyczne odstępy).

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use agent_backends_contract::contract_tests::scenario::{Scenario, permission_result};
use agent_backends_contract::{
    AgentBackend, AgentEvent, AgentEventEnvelope, AgentEventStream, ApprovalDecision, ApprovalSink,
    BackendError, BridgeKind, LaunchPolicy, OutputFormat, PermissionKind, PermissionRequest,
    PermissionRequestId, SessionRef, TaskHandle, TaskId, TaskResult, TaskSpec, check_origin,
};
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot, watch};

#[derive(Default)]
struct Log {
    events: Vec<AgentEventEnvelope>,
    finished: bool,
}

struct Task {
    log: Arc<Mutex<Log>>,
    tick: watch::Sender<usize>,
    cancel: watch::Sender<bool>,
    steer: mpsc::UnboundedSender<String>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Clone)]
struct Emitter {
    task: TaskId,
    log: Arc<Mutex<Log>>,
    tick: watch::Sender<usize>,
}

impl Emitter {
    fn emit(&self, event: AgentEvent) {
        let len = {
            let mut log = lock(&self.log);
            if log.finished {
                return;
            }
            log.finished = event.is_terminal();
            let seq = log.events.len() as u64;
            log.events
                .push(AgentEventEnvelope::new(self.task.clone(), seq, seq, event));
            log.events.len()
        };
        self.tick.send_replace(len);
    }
}

#[derive(Default)]
struct State {
    tasks: HashMap<TaskId, Task>,
    pending: HashMap<PermissionRequestId, oneshot::Sender<ApprovalDecision>>,
    disabled: BTreeSet<BridgeKind>,
    submitted: Vec<TaskSpec>,
}

/// Atrapa `AgentBackend`.
pub struct FakeAgentBackend {
    sink: Arc<dyn ApprovalSink>,
    policy: LaunchPolicy,
    state: Arc<Mutex<State>>,
    counter: AtomicU64,
}

impl FakeAgentBackend {
    /// Atrapa z kanałem zatwierdzeń.
    pub fn new(sink: Arc<dyn ApprovalSink>) -> Self {
        Self {
            sink,
            policy: LaunchPolicy::default(),
            state: Arc::new(Mutex::new(State::default())),
            counter: AtomicU64::new(0),
        }
    }

    /// Polityka uruchamiania (zgody na harmonogram).
    #[must_use]
    pub fn with_policy(mut self, policy: LaunchPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Symuluje trasę wyłączoną w `compliance`.
    pub fn disable_route(&self, bridge: BridgeKind) {
        lock(&self.state).disabled.insert(bridge);
    }

    /// Przyjęte zadania (po bramce).
    pub fn submitted(&self) -> Vec<TaskSpec> {
        lock(&self.state).submitted.clone()
    }
}

struct Run {
    em: Emitter,
    sink: Arc<dyn ApprovalSink>,
    state: Arc<Mutex<State>>,
    bridge: BridgeKind,
    steer: mpsc::UnboundedReceiver<String>,
    counter: u64,
}

impl Run {
    fn output(&self, text: impl Into<String>) {
        self.em.emit(AgentEvent::Output {
            format: OutputFormat::Markdown,
            text: text.into(),
            partial: false,
        });
    }

    fn done(&self, text: impl Into<String>, is_error: bool) {
        self.em.emit(AgentEvent::Done {
            result: TaskResult {
                text: text.into(),
                is_error,
                subtype: Some(if is_error { "error" } else { "success" }.into()),
                session: Some(SessionRef {
                    bridge: self.bridge,
                    id: format!("fake-{}", self.em.task),
                    workdir: PathBuf::from("/fake/worktree"),
                }),
                num_turns: Some(1),
                duration_ms: Some(1),
            },
        });
    }

    async fn ask(&mut self, i: u32) -> bool {
        self.counter += 1;
        let id = PermissionRequestId(format!("{}-perm-{}", self.em.task, self.counter));
        let (tx, rx) = oneshot::channel();
        lock(&self.state).pending.insert(id.clone(), tx);
        let request = PermissionRequest {
            id: id.clone(),
            task: self.em.task.clone(),
            bridge: self.bridge,
            kind: PermissionKind::Tool,
            tool: "Bash".into(),
            input: serde_json::json!({"command": format!("echo {i}")}),
            reason: None,
            call_id: Some(format!("toolu_{i}")),
        };
        self.em.emit(AgentEvent::PermissionRequest {
            request: request.clone(),
        });
        let decision = match self.sink.request(request).await {
            Some(d) => d,
            None => rx
                .await
                .unwrap_or_else(|_| ApprovalDecision::deny("anulowano")),
        };
        lock(&self.state).pending.remove(&id);
        let allow = decision.is_allow();
        self.em.emit(AgentEvent::PermissionResolved {
            id,
            decision,
            timed_out: false,
        });
        allow
    }

    async fn play(mut self, scenario: Scenario) {
        self.em.emit(AgentEvent::ColdStart { ms: 0 });
        match scenario {
            Scenario::Permission(n) => {
                let mut decisions = Vec::new();
                for i in 0..n {
                    decisions.push(self.ask(i).await);
                }
                self.done(permission_result(&decisions), false);
            }
            Scenario::Slow { n, interval_ms } => {
                for i in 0..n {
                    self.output(format!("krok {i}"));
                    tokio::time::sleep(Duration::from_millis(interval_ms)).await;
                }
                self.done("ok", false);
            }
            Scenario::Hang => {
                self.output("czekam");
                std::future::pending::<()>().await;
            }
            Scenario::Crash => {
                self.output("zaczynam");
                self.em.emit(AgentEvent::Error {
                    error: BackendError::CliExited {
                        code: Some(3),
                        stderr_tail: String::new(),
                    },
                });
            }
            Scenario::ErrorResult => self.done("limit tur", true),
            Scenario::Garbage | Scenario::LongLine(_) => {
                self.em.emit(AgentEvent::Warning {
                    message: "linia spoza protokołu".into(),
                });
                self.done("ok", false);
            }
            Scenario::Env => {
                self.output("PATH");
                self.done("ok", false);
            }
            Scenario::Steer => {
                self.output("czekam na sterowanie");
                let msg = self.steer.recv().await.unwrap_or_default();
                self.output(format!("steer:{msg}"));
                self.done(format!("steer:{msg}"), false);
            }
            Scenario::Ok => {
                self.output("Planuję.");
                self.done("Gotowe.", false);
            }
        }
    }
}

#[async_trait]
impl AgentBackend for FakeAgentBackend {
    async fn submit_task(&self, spec: TaskSpec) -> Result<TaskHandle, BackendError> {
        check_origin(&spec.origin, spec.bridge, &self.policy, 0)
            .map_err(|refusal| BackendError::LaunchRefused { refusal })?;
        if lock(&self.state).disabled.contains(&spec.bridge) {
            return Err(BackendError::RouteNotAllowed {
                route: spec.bridge.route_id_str().into(),
                reason: "trasa wyłączona".into(),
            });
        }
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let task = TaskId(format!("fake-task-{n}"));
        let workdir = PathBuf::from(format!("/fake/worktree/{n}"));
        let log = Arc::new(Mutex::new(Log::default()));
        let (tick, _) = watch::channel(0);
        let (cancel, mut cancelled) = watch::channel(false);
        let (steer_tx, steer_rx) = mpsc::unbounded_channel();
        let em = Emitter {
            task: task.clone(),
            log: log.clone(),
            tick: tick.clone(),
        };
        em.emit(AgentEvent::Started {
            bridge: spec.bridge,
            cli_version: "0.0.0-fake".into(),
            workdir: workdir.clone(),
        });
        {
            let mut st = lock(&self.state);
            st.submitted.push(spec.clone());
            st.tasks.insert(
                task.clone(),
                Task {
                    log,
                    tick,
                    cancel,
                    steer: steer_tx,
                },
            );
        }
        let run = Run {
            em: em.clone(),
            sink: self.sink.clone(),
            state: self.state.clone(),
            bridge: spec.bridge,
            steer: steer_rx,
            counter: 0,
        };
        let scenario = Scenario::parse(&spec.prompt);
        tokio::spawn(async move {
            tokio::select! {
                () = run.play(scenario) => {}
                _ = cancelled.wait_for(|c| *c) => em.emit(AgentEvent::Error { error: BackendError::Cancelled }),
            }
        });
        Ok(TaskHandle {
            task,
            bridge: spec.bridge,
            workdir,
        })
    }

    fn events(&self, task: &TaskId) -> Result<AgentEventStream, BackendError> {
        let st = lock(&self.state);
        let t = st
            .tasks
            .get(task)
            .ok_or_else(|| BackendError::UnknownTask(task.0.clone()))?;
        let state = (t.log.clone(), t.tick.subscribe(), 0usize);
        Ok(Box::pin(futures_util::stream::unfold(
            state,
            |(log, mut rx, idx)| async move {
                loop {
                    let (next, finished) = {
                        let l = lock(&log);
                        (l.events.get(idx).cloned(), l.finished)
                    };
                    if let Some(ev) = next {
                        return Some((ev, (log, rx, idx + 1)));
                    }
                    if finished || rx.changed().await.is_err() {
                        return None;
                    }
                }
            },
        )))
    }

    async fn approve(
        &self,
        request: &PermissionRequestId,
        decision: ApprovalDecision,
    ) -> Result<(), BackendError> {
        let tx = lock(&self.state)
            .pending
            .remove(request)
            .ok_or_else(|| BackendError::UnknownPermissionRequest(request.0.clone()))?;
        tx.send(decision)
            .map_err(|_| BackendError::UnknownPermissionRequest(request.0.clone()))
    }

    async fn steer(&self, task: &TaskId, message: String) -> Result<(), BackendError> {
        let st = lock(&self.state);
        let t = st
            .tasks
            .get(task)
            .ok_or_else(|| BackendError::UnknownTask(task.0.clone()))?;
        if lock(&t.log).finished {
            return Err(BackendError::TaskFinished);
        }
        t.steer
            .send(message)
            .map_err(|_| BackendError::SteeringUnavailable("zadanie kończy się".into()))
    }

    async fn cancel(&self, task: &TaskId) -> Result<(), BackendError> {
        let st = lock(&self.state);
        let t = st
            .tasks
            .get(task)
            .ok_or_else(|| BackendError::UnknownTask(task.0.clone()))?;
        if lock(&t.log).finished {
            return Err(BackendError::TaskFinished);
        }
        t.cancel.send_replace(true);
        Ok(())
    }
}
