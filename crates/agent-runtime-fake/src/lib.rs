//! Atrapa `agent-runtime` (docs/modules/agent-runtime/SPEC.md, sekcja „Fake”): przebieg
//! odtwarza skrypt zdarzeń z opóźnieniami (`tokio::time` — w testach z `start_paused`
//! deterministycznie). Sterowanie daje `Steered`/`Paused`/`Resumed`, anulowanie kończy
//! przebieg `Cancelled`; skrypt bez `Finished` trwa do anulowania. Do testów UI, Replay,
//! `voice-dialog` i `broker-ui` (karty „czeka na zatwierdzenie”).
//!
//! v1: `start_with` przyjmuje opcje (zapisywane do asercji: [`FakeAgentRuntime::options`]);
//! przebieg z `options.parent` jest podprzebiegiem rodzica (`children`, raport z dziećmi).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use agent_runtime_contract::{
    AgentRuntime, RunError, RunEvent, RunEventEnvelope, RunId, RunOptions, RunOutcome, RunSpec,
    RunStatus, Steer,
};
use async_trait::async_trait;
use tokio::sync::{Notify, broadcast, watch};
use tokio_util::sync::CancellationToken;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Skrypt przebiegu: zdarzenia z odstępami.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunScript {
    /// (opóźnienie przed zdarzeniem, zdarzenie).
    pub items: Vec<(Duration, RunEvent)>,
}

impl RunScript {
    /// Pusty skrypt (przebieg trwa do anulowania).
    pub fn new() -> Self {
        Self::default()
    }

    /// Dopisuje zdarzenie po `ms` milisekundach.
    #[must_use]
    pub fn then(mut self, ms: u64, event: RunEvent) -> Self {
        self.items.push((Duration::from_millis(ms), event));
        self
    }

    /// Kończy przebieg wynikiem po `ms` milisekundach.
    #[must_use]
    pub fn finish(self, ms: u64, outcome: RunOutcome) -> Self {
        self.then(ms, RunEvent::Finished { outcome })
    }
}

struct FakeRunState {
    cancel: CancellationToken,
    wake: Notify,
    steer: Mutex<VecDeque<Steer>>,
    log: Mutex<Vec<RunEventEnvelope>>,
    status: Mutex<RunStatus>,
    tx: broadcast::Sender<RunEventEnvelope>,
    outcome: watch::Sender<Option<RunOutcome>>,
    active: AtomicBool,
    started: tokio::time::Instant,
    run: RunId,
}

impl FakeRunState {
    fn record(&self, event: RunEvent) {
        let mut log = lock(&self.log);
        let env = RunEventEnvelope {
            run: self.run.clone(),
            seq: log.len() as u64 + 1,
            at_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
            event,
        };
        log.push(env.clone());
        drop(log);
        let mut st = lock(&self.status);
        *st = st.clone().apply(&env.event);
        drop(st);
        if let RunEvent::Finished { outcome } = &env.event {
            self.active.store(false, Ordering::SeqCst);
            self.outcome.send_replace(Some(outcome.clone()));
        }
        let _ = self.tx.send(env);
    }

    /// Obsługa sterowania; `true` = pauza aktywna.
    fn steer(&self, paused: bool) -> bool {
        let mut paused = paused;
        for s in lock(&self.steer).drain(..).collect::<Vec<_>>() {
            match s {
                Steer::Message(m) | Steer::ChangeGoal(m) => {
                    self.record(RunEvent::Steered { message: m })
                }
                Steer::PauseAfterCurrent if !paused => {
                    paused = true;
                    self.record(RunEvent::Paused);
                }
                Steer::Resume if paused => {
                    paused = false;
                    self.record(RunEvent::Resumed);
                }
                _ => {}
            }
        }
        paused
    }

    async fn play(self: Arc<Self>, spec: RunSpec, script: RunScript) {
        self.record(RunEvent::Started {
            goal: spec.goal,
            tools: spec.tools,
            budget: spec.budget,
        });
        let mut items: VecDeque<(Duration, RunEvent)> = script.items.into();
        let mut paused = false;
        loop {
            paused = self.steer(paused);
            if self.cancel.is_cancelled() {
                return self.record(RunEvent::Finished {
                    outcome: RunOutcome::Cancelled,
                });
            }
            let next = if paused { None } else { items.pop_front() };
            match next {
                Some((delay, event)) => {
                    let deadline = tokio::time::Instant::now() + delay;
                    let mut requeue = false;
                    loop {
                        tokio::select! {
                            () = tokio::time::sleep_until(deadline) => break,
                            () = self.cancel.cancelled() => {
                                return self.record(RunEvent::Finished { outcome: RunOutcome::Cancelled });
                            }
                            () = self.wake.notified() => {
                                paused = self.steer(paused);
                                if paused {
                                    requeue = true;
                                    break;
                                }
                            }
                        }
                    }
                    if requeue {
                        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
                        items.push_front((left, event));
                        continue;
                    }
                    let done = matches!(event, RunEvent::Finished { .. });
                    self.record(event);
                    if done {
                        return;
                    }
                }
                None => {
                    tokio::select! {
                        () = self.wake.notified() => {}
                        () = self.cancel.cancelled() => {}
                    }
                }
            }
        }
    }
}

/// Atrapa runtime: kolejka skryptów (FIFO) i skrypt domyślny.
#[derive(Default)]
pub struct FakeAgentRuntime {
    scripts: Mutex<VecDeque<RunScript>>,
    default: Mutex<RunScript>,
    runs: Mutex<BTreeMap<RunId, Arc<FakeRunState>>>,
    steers: Mutex<Vec<(RunId, Steer)>>,
    options: Mutex<Vec<(RunId, RunOptions)>>,
    next: AtomicU64,
}

impl FakeAgentRuntime {
    /// Atrapa bez skryptów (przebiegi trwają do anulowania).
    pub fn new() -> Self {
        Self::default()
    }

    /// Skrypt najbliższego przebiegu.
    pub fn push_script(&self, script: RunScript) {
        lock(&self.scripts).push_back(script);
    }

    /// Skrypt, gdy kolejka jest pusta.
    pub fn set_default(&self, script: RunScript) {
        *lock(&self.default) = script;
    }

    /// Przyjęte sterowania (do asercji).
    pub fn steers(&self) -> Vec<(RunId, Steer)> {
        lock(&self.steers).clone()
    }

    /// Opcje, z którymi wystartował przebieg (do asercji).
    pub fn options(&self, run: &RunId) -> Option<RunOptions> {
        lock(&self.options)
            .iter()
            .find(|(r, _)| r == run)
            .map(|(_, o)| o.clone())
    }

    fn state(&self, run: &RunId) -> Result<Arc<FakeRunState>, RunError> {
        lock(&self.runs)
            .get(run)
            .cloned()
            .ok_or_else(|| RunError::UnknownRun(run.clone()))
    }
}

#[async_trait]
impl AgentRuntime for FakeAgentRuntime {
    async fn start(&self, spec: RunSpec) -> Result<RunId, RunError> {
        self.start_with(spec, RunOptions::default()).await
    }

    async fn start_with(&self, spec: RunSpec, options: RunOptions) -> Result<RunId, RunError> {
        spec.validate().map_err(RunError::InvalidSpec)?;
        if let Some(parent) = &options.parent {
            self.state(parent)?;
        }
        let n = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let run = RunId::new(format!("fake-run-{n}"));
        let script = lock(&self.scripts)
            .pop_front()
            .unwrap_or_else(|| lock(&self.default).clone());
        let (tx, _) = broadcast::channel(1024);
        let (outcome, _) = watch::channel(None);
        let state = Arc::new(FakeRunState {
            cancel: CancellationToken::new(),
            wake: Notify::new(),
            steer: Mutex::new(VecDeque::new()),
            log: Mutex::new(Vec::new()),
            status: Mutex::new(RunStatus::Running { step: 0 }),
            tx,
            outcome,
            active: AtomicBool::new(true),
            started: tokio::time::Instant::now(),
            run: run.clone(),
        });
        lock(&self.runs).insert(run.clone(), state.clone());
        lock(&self.options).push((run.clone(), options));
        tokio::spawn(state.play(spec, script));
        Ok(run)
    }

    fn steer(&self, run: &RunId, steer: Steer) -> Result<(), RunError> {
        let st = self.state(run)?;
        if !st.active.load(Ordering::SeqCst) {
            return Err(RunError::AlreadyFinished(run.clone()));
        }
        lock(&self.steers).push((run.clone(), steer.clone()));
        if steer == Steer::Cancel {
            st.cancel.cancel();
        }
        lock(&st.steer).push_back(steer);
        st.wake.notify_one();
        Ok(())
    }

    fn cancel(&self, run: &RunId) -> Result<(), RunError> {
        let st = self.state(run)?;
        st.cancel.cancel();
        st.wake.notify_one();
        Ok(())
    }

    async fn resume(&self, run: &RunId) -> Result<(), RunError> {
        match lock(&self.runs).get(run) {
            Some(s) if s.active.load(Ordering::SeqCst) => {
                Err(RunError::AlreadyRunning(run.clone()))
            }
            Some(_) => Err(RunError::AlreadyFinished(run.clone())),
            None => Err(RunError::NoCheckpoint(run.clone())),
        }
    }

    async fn wait(&self, run: &RunId) -> Result<RunOutcome, RunError> {
        let mut rx = self.state(run)?.outcome.subscribe();
        loop {
            if let Some(o) = rx.borrow_and_update().clone() {
                return Ok(o);
            }
            if rx.changed().await.is_err() {
                return Err(RunError::UnknownRun(run.clone()));
            }
        }
    }

    fn status(&self, run: &RunId) -> Result<RunStatus, RunError> {
        Ok(lock(&self.state(run)?.status).clone())
    }

    fn events(&self, run: &RunId) -> Result<Vec<RunEventEnvelope>, RunError> {
        Ok(lock(&self.state(run)?.log).clone())
    }

    fn subscribe(&self, run: &RunId) -> Result<broadcast::Receiver<RunEventEnvelope>, RunError> {
        Ok(self.state(run)?.tx.subscribe())
    }

    fn children(&self, run: &RunId) -> Result<Vec<RunId>, RunError> {
        self.state(run)?;
        Ok(lock(&self.options)
            .iter()
            .filter(|(_, o)| o.parent.as_ref() == Some(run))
            .map(|(r, _)| r.clone())
            .collect())
    }
}
