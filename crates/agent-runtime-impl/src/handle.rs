//! Uchwyt przebiegu współdzielony przez API runtime i pętlę: dziennik zdarzeń (Replay),
//! subskrypcje UI, kolejka sterowania, anulowanie, stan, wynik.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use agent_runtime_contract::{RunEvent, RunEventEnvelope, RunId, RunOutcome, RunStatus, Steer};
use core_bus_contract::{AgentId, EventBus, SessionId};
use tokio::sync::{Notify, broadcast, watch};
use tokio_util::sync::CancellationToken;

/// Pojemność kanału subskrypcji (wolny subskrybent dostaje `Lagged`, dziennik jest pełny).
const SUBSCRIBERS_CAPACITY: usize = 1024;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub(crate) struct RunHandle {
    pub(crate) run: RunId,
    pub(crate) session: SessionId,
    pub(crate) agent: AgentId,
    pub(crate) cancel: CancellationToken,
    pub(crate) wake: Notify,
    steer: Mutex<VecDeque<Steer>>,
    log: Mutex<Vec<RunEventEnvelope>>,
    tx: broadcast::Sender<RunEventEnvelope>,
    status: Mutex<RunStatus>,
    outcome: watch::Sender<Option<RunOutcome>>,
    started: tokio::time::Instant,
    bus: Option<Arc<dyn EventBus>>,
    pub(crate) active: AtomicBool,
}

impl RunHandle {
    pub(crate) fn new(
        run: RunId,
        session: SessionId,
        agent: AgentId,
        bus: Option<Arc<dyn EventBus>>,
    ) -> Self {
        let (tx, _) = broadcast::channel(SUBSCRIBERS_CAPACITY);
        let (outcome, _) = watch::channel(None);
        Self {
            run,
            session,
            agent,
            cancel: CancellationToken::new(),
            wake: Notify::new(),
            steer: Mutex::new(VecDeque::new()),
            log: Mutex::new(Vec::new()),
            tx,
            status: Mutex::new(RunStatus::Running { step: 0 }),
            outcome,
            started: tokio::time::Instant::now(),
            bus,
            active: AtomicBool::new(true),
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

    pub(crate) fn push_steer(&self, s: Steer) {
        if s == Steer::Cancel {
            self.cancel.cancel();
        }
        lock(&self.steer).push_back(s);
        self.wake.notify_one();
    }

    pub(crate) fn drain_steer(&self) -> Vec<Steer> {
        lock(&self.steer).drain(..).collect()
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
        self.active.store(false, Ordering::SeqCst);
        self.outcome.send_replace(Some(outcome));
    }

    pub(crate) fn outcome_watch(&self) -> watch::Receiver<Option<RunOutcome>> {
        self.outcome.subscribe()
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
}
