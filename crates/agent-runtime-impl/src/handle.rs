//! Uchwyt przebiegu współdzielony przez API runtime i pętlę: dziennik zdarzeń (Replay),
//! subskrypcje UI, kolejka sterowania (operacje vs treść), anulowanie, stan (działa / oddany
//! schedulerowi / zakończony), podprzebiegi (delegacje, Krytyczka), wynik.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use agent_runtime_contract::{RunEvent, RunEventEnvelope, RunId, RunOutcome, RunStatus, Steer};
use core_bus_contract::{AgentId, EventBus, SessionId};
use tokio::sync::{Notify, broadcast, watch};
use tokio_util::sync::CancellationToken;

/// Pojemność kanału subskrypcji (wolny subskrybent dostaje `Lagged`, dziennik jest pełny).
const SUBSCRIBERS_CAPACITY: usize = 1024;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Sterowanie w kolejce (głos/tekst — różni się tylko znacznikiem w prompcie).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Queued {
    pub(crate) steer: Steer,
    pub(crate) voice: bool,
}

fn is_content(s: &Steer) -> bool {
    matches!(s, Steer::Message(_) | Steer::ChangeGoal(_))
}

pub(crate) struct RunHandle {
    pub(crate) run: RunId,
    pub(crate) session: SessionId,
    pub(crate) agent: AgentId,
    pub(crate) cancel: CancellationToken,
    pub(crate) wake: Notify,
    steer: Mutex<VecDeque<Queued>>,
    log: Mutex<Vec<RunEventEnvelope>>,
    tx: broadcast::Sender<RunEventEnvelope>,
    status: Mutex<RunStatus>,
    outcome: watch::Sender<Option<RunOutcome>>,
    started: tokio::time::Instant,
    bus: Option<Arc<dyn EventBus>>,
    finished: AtomicBool,
    running: AtomicBool,
    children: Mutex<Vec<RunId>>,
    active_child: Mutex<Option<Arc<RunHandle>>>,
}

impl RunHandle {
    pub(crate) fn new(
        run: RunId,
        session: SessionId,
        agent: AgentId,
        bus: Option<Arc<dyn EventBus>>,
        cancel: CancellationToken,
    ) -> Self {
        let (tx, _) = broadcast::channel(SUBSCRIBERS_CAPACITY);
        let (outcome, _) = watch::channel(None);
        Self {
            run,
            session,
            agent,
            cancel,
            wake: Notify::new(),
            steer: Mutex::new(VecDeque::new()),
            log: Mutex::new(Vec::new()),
            tx,
            status: Mutex::new(RunStatus::Running { step: 0 }),
            outcome,
            started: tokio::time::Instant::now(),
            bus,
            finished: AtomicBool::new(false),
            running: AtomicBool::new(true),
            children: Mutex::new(Vec::new()),
            active_child: Mutex::new(None),
        }
    }

    /// Zapisuje zdarzenie (dziennik, subskrybenci, stan) — synchronicznie.
    pub(crate) fn record(&self, event: RunEvent) -> RunEventEnvelope {
        let mut log = lock(&self.log);
        let env = RunEventEnvelope {
            run: self.run.clone(),
            seq: log.len() as u64 + 1,
            at_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
            event,
        };
        log.push(env.clone());
        drop(log);
        let mut status = lock(&self.status);
        *status = status.clone().apply(&env.event);
        drop(status);
        let _ = self.tx.send(env.clone());
        env
    }

    /// Zapis + publikacja na magistrali (best effort).
    pub(crate) async fn emit(&self, event: RunEvent) {
        let env = self.record(event);
        if let Some(bus) = &self.bus {
            let _ = bus
                .publish(env.to_bus_event(&self.session, &self.agent))
                .await;
        }
    }

    /// Zapis z publikacją w tle (z kontekstu synchronicznego, np. obserwator narzędzia).
    pub(crate) fn emit_detached(&self, event: RunEvent) {
        let env = self.record(event);
        if let Some(bus) = self.bus.clone() {
            let ev = env.to_bus_event(&self.session, &self.agent);
            tokio::spawn(async move {
                let _ = bus.publish(ev).await;
            });
        }
    }

    /// Sterowanie: kolejka + przekazanie do aktywnego podprzebiegu (≤ 1 krok także w delegacji).
    pub(crate) fn push_steer(&self, steer: Steer, voice: bool) {
        if steer == Steer::Cancel {
            self.cancel.cancel();
        } else if let Some(child) = self.active_child() {
            child.push_steer(steer.clone(), voice);
        }
        lock(&self.steer).push_back(Queued { steer, voice });
        self.wake.notify_one();
    }

    /// Całe sterowanie (punkt atomowy przed turą modelu).
    pub(crate) fn drain_steer(&self) -> Vec<Queued> {
        lock(&self.steer).drain(..).collect()
    }

    /// Tylko operacje (pauza, wznowienie, anulowanie) — treść zostaje na następną turę.
    pub(crate) fn take_ops(&self) -> Vec<Steer> {
        let mut q = lock(&self.steer);
        let (content, ops): (VecDeque<Queued>, VecDeque<Queued>) =
            q.drain(..).partition(|s| is_content(&s.steer));
        *q = content;
        ops.into_iter().map(|s| s.steer).collect()
    }

    /// Czy w kolejce czeka treść dla agentki (wiadomość, nowy cel).
    pub(crate) fn has_content(&self) -> bool {
        lock(&self.steer).iter().any(|s| is_content(&s.steer))
    }

    pub(crate) fn status(&self) -> RunStatus {
        lock(&self.status).clone()
    }

    pub(crate) fn events(&self) -> Vec<RunEventEnvelope> {
        lock(&self.log).clone()
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<RunEventEnvelope> {
        self.tx.subscribe()
    }

    pub(crate) fn set_outcome(&self, outcome: RunOutcome) {
        self.finished.store(true, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        self.outcome.send_replace(Some(outcome));
    }

    /// Porzucenie przebiegu oddanego schedulerowi (podprzebieg, gdy rodzic oddaje zadanie):
    /// koniec jako anulowany, bez dalszego wznawiania.
    pub(crate) fn abandon(&self) {
        if self.is_active() {
            self.record(RunEvent::Finished {
                outcome: RunOutcome::Cancelled,
            });
            self.set_outcome(RunOutcome::Cancelled);
        }
    }

    pub(crate) fn outcome(&self) -> Option<RunOutcome> {
        self.outcome.borrow().clone()
    }

    pub(crate) fn outcome_watch(&self) -> watch::Receiver<Option<RunOutcome>> {
        self.outcome.subscribe()
    }

    /// Przyjmuje sterowanie (nie zakończony — także oddany schedulerowi).
    pub(crate) fn is_active(&self) -> bool {
        !self.finished.load(Ordering::SeqCst)
    }

    /// Pętla działa.
    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub(crate) fn set_running(&self, running: bool) {
        self.running.store(running, Ordering::SeqCst);
    }

    pub(crate) fn add_child(&self, child: &Arc<RunHandle>) {
        lock(&self.children).push(child.run.clone());
        *lock(&self.active_child) = Some(child.clone());
    }

    pub(crate) fn clear_active_child(&self) {
        *lock(&self.active_child) = None;
    }

    fn active_child(&self) -> Option<Arc<RunHandle>> {
        lock(&self.active_child).clone()
    }

    pub(crate) fn children(&self) -> Vec<RunId> {
        lock(&self.children).clone()
    }
}
