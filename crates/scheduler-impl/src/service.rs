//! Sterownik produkcyjny wokół `SchedCore`: zegar (epoka + tokio `Instant`), timer terminów,
//! wykonawczynie jako zadania tokio (przerywalne), publikacja zdarzeń na magistralę i zapis
//! stanu w tle (łączenie zapisów).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use core_bus_contract::{Event, EventBus, Level};
use cost_meter_contract::BudgetDecision;
use scheduler_contract::{
    CancelCause, Dispatch, DispatchId, SchedCore, SchedEffect, SchedHost, Snapshot, SnapshotStore,
    StepDirective, StepGate, StepReport, StopReason, TaskError, TaskExecutor, TaskId, TaskSpec,
    WorkerResult, event_kind,
};
use serde_json::json;
use tokio::sync::{Notify, mpsc};
use tokio::task::{AbortHandle, JoinHandle};
use tokio::time::Instant;

use crate::ports::BackgroundBudget;

/// Zdarzenie: nie udało się zapisać stanu (scheduler działa dalej, ostrzega Diagnostę).
pub const EVENT_PERSIST_FAILED: &str = "scheduler.persist_failed";

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Otoczenie produkcyjne.
pub struct ImplHost {
    epoch_ms: u64,
    start: Instant,
    events: mpsc::UnboundedSender<Vec<Event>>,
    wake: Arc<Notify>,
    pending: Mutex<Option<Snapshot>>,
    persist_wake: Arc<Notify>,
    budget: Arc<dyn BackgroundBudget>,
}

impl SchedHost for ImplHost {
    fn now_ms(&self) -> u64 {
        self.epoch_ms.saturating_add(millis(self.start.elapsed()))
    }

    fn emit(&self, events: Vec<Event>) {
        // Zamknięty kanał = moduł zatrzymany; zdarzenia diagnostyczne można pominąć.
        let _ = self.events.send(events);
    }

    fn wake(&self) {
        self.wake.notify_one();
    }

    fn persist(&self, snapshot: &Snapshot) {
        *lock(&self.pending) = Some(snapshot.clone());
        self.persist_wake.notify_one();
    }

    fn check_background_budget(&self, estimate_micro_pln: u64) -> BudgetDecision {
        self.budget.check(estimate_micro_pln)
    }
}

/// Uruchomiony scheduler.
pub(crate) struct Service {
    pub(crate) core: Arc<SchedCore<ImplHost>>,
    executor: Arc<dyn TaskExecutor>,
    store: Arc<dyn SnapshotStore>,
    workers: Mutex<BTreeMap<DispatchId, AbortHandle>>,
    pub(crate) finished: Notify,
    background: Mutex<Vec<JoinHandle<()>>>,
    /// Zapisy magazynu po kolei; `true` = moduł zatrzymany (zapis w tle po zapisie końcowym
    /// nadpisałby nowszy stan starszym — zapis z `spawn_blocking` nie da się przerwać).
    saves: Arc<Mutex<bool>>,
    me: Weak<Service>,
}

impl Service {
    /// Start: stan z magazynu (wznowienie) albo pusty, zadania tła, pierwszy przegląd.
    pub(crate) fn start(
        bus: Arc<dyn EventBus>,
        executor: Arc<dyn TaskExecutor>,
        store: Arc<dyn SnapshotStore>,
        budget: Arc<dyn BackgroundBudget>,
    ) -> Result<Arc<Self>, String> {
        let (tx, rx) = mpsc::unbounded_channel();
        let wake = Arc::new(Notify::new());
        let persist_wake = Arc::new(Notify::new());
        let epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, millis);
        let host = Arc::new(ImplHost {
            epoch_ms,
            start: Instant::now(),
            events: tx,
            wake: Arc::clone(&wake),
            pending: Mutex::new(None),
            persist_wake: Arc::clone(&persist_wake),
            budget,
        });
        let core = match store.load()? {
            Some(snapshot) => SchedCore::restore(host, snapshot)?,
            None => SchedCore::new(host),
        };
        let svc = Arc::new_cyclic(|me| Self {
            core,
            executor,
            store,
            workers: Mutex::new(BTreeMap::new()),
            finished: Notify::new(),
            background: Mutex::new(Vec::new()),
            saves: Arc::default(),
            me: me.clone(),
        });
        let tasks = vec![
            tokio::spawn(publisher(bus, rx)),
            tokio::spawn(driver(Arc::downgrade(&svc), wake)),
            tokio::spawn(persister(Arc::downgrade(&svc), persist_wake)),
        ];
        *lock(&svc.background) = tasks;
        svc.apply(svc.core.pump());
        Ok(svc)
    }

    /// Zatrzymanie łagodne: stan zapisany (zadania w toku wznowią się po starcie), wykonawczynie
    /// i zadania tła przerwane.
    pub(crate) fn shutdown(&self) -> Result<(), String> {
        for task in lock(&self.background).drain(..) {
            task.abort();
        }
        for (_, worker) in std::mem::take(&mut *lock(&self.workers)) {
            worker.abort();
        }
        let mut stopped = lock(&self.saves);
        *stopped = true;
        self.store.save(&self.core.snapshot())
    }

    /// Wykonuje efekty rdzenia (poza jego blokadą, w kolejności).
    pub(crate) fn apply(&self, effects: Vec<SchedEffect>) {
        let mut finished = false;
        for effect in effects {
            match effect {
                SchedEffect::Dispatch(d) => self.spawn_worker(*d),
                SchedEffect::Abort { dispatch, .. } => {
                    if let Some(worker) = lock(&self.workers).remove(&dispatch) {
                        worker.abort();
                    }
                }
                SchedEffect::Finished { .. } => finished = true,
            }
        }
        if finished {
            self.finished.notify_waiters();
        }
    }

    fn spawn_worker(&self, dispatch: Dispatch) {
        let id = dispatch.dispatch;
        let gate: Arc<dyn StepGate> = Arc::new(Gate {
            svc: self.me.clone(),
            dispatch: id,
        });
        let executor = Arc::clone(&self.executor);
        let handle = tokio::spawn(async move { executor.execute(dispatch, gate).await });
        lock(&self.workers).insert(id, handle.abort_handle());
        let weak = self.me.clone();
        // Nadzorca: wynik albo awaria wykonawczyni (panika = błąd nieponawialny); przerwanie
        // (`Abort`) — rdzeń już zdecydował.
        drop(tokio::spawn(async move {
            let result = match handle.await {
                Ok(result) => Some(result),
                Err(e) if e.is_panic() => Some(WorkerResult::Failed {
                    error: "wykonawczyni uległa awarii".into(),
                    retryable: false,
                }),
                Err(_) => None,
            };
            if let Some(svc) = weak.upgrade() {
                lock(&svc.workers).remove(&id);
                if let Some(result) = result {
                    let effects = svc.core.finish(id, result);
                    svc.apply(effects);
                }
            }
        }));
    }
}

/// Punkt atomowy dla wykonawczyni.
struct Gate {
    svc: Weak<Service>,
    dispatch: DispatchId,
}

impl StepGate for Gate {
    fn boundary(&self, report: StepReport) -> StepDirective {
        let Some(svc) = self.svc.upgrade() else {
            return StepDirective::Stop {
                reason: StopReason::Cancelled {
                    cause: CancelCause::KillSwitch,
                },
            };
        };
        let (directive, effects) = svc.core.boundary(self.dispatch, &report);
        svc.apply(effects);
        directive
    }

    fn spawn(&self, tasks: Vec<TaskSpec>) -> Result<Vec<TaskId>, TaskError> {
        let svc = self.svc.upgrade().ok_or(TaskError::NotStarted)?;
        let (ids, effects) = svc.core.spawn(self.dispatch, tasks)?;
        svc.apply(effects);
        Ok(ids)
    }
}

/// Timer: śpi do najbliższego terminu albo do zmiany stanu, potem `tick` + `pump`.
async fn driver(svc: Weak<Service>, wake: Arc<Notify>) {
    loop {
        let Some((next, now)) = svc
            .upgrade()
            .map(|s| (s.core.next_wake(), s.core.host().now_ms()))
        else {
            return;
        };
        match next {
            Some(at) => {
                let sleep = tokio::time::sleep(Duration::from_millis(at.saturating_sub(now)));
                tokio::select! {
                    () = sleep => {}
                    () = wake.notified() => {}
                }
            }
            None => wake.notified().await,
        }
        let Some(s) = svc.upgrade() else {
            return;
        };
        // `tick` tylko po terminie warstwy zasobów (każda operacja na niej budzi sterownik).
        let now = s.core.host().now_ms();
        if s.core.locks().next_deadline().is_some_and(|d| d <= now) {
            s.core.locks().tick();
        }
        let effects = s.core.pump();
        s.apply(effects);
    }
}

/// Publikacja zdarzeń (asynchroniczna; decyzje nie czekają na magistralę).
async fn publisher(bus: Arc<dyn EventBus>, mut rx: mpsc::UnboundedReceiver<Vec<Event>>) {
    while let Some(events) = rx.recv().await {
        for event in events {
            // Błąd magistrali nie cofa decyzji.
            let _ = bus.publish(event).await;
        }
    }
}

/// Zapis stanu w tle: zawsze najnowszy, zapisy łączone.
async fn persister(svc: Weak<Service>, wake: Arc<Notify>) {
    loop {
        wake.notified().await;
        let Some(s) = svc.upgrade() else {
            return;
        };
        let Some(snapshot) = lock(&s.core.host().pending).take() else {
            continue;
        };
        let (store, saves) = (Arc::clone(&s.store), Arc::clone(&s.saves));
        drop(s);
        let saved = tokio::task::spawn_blocking(move || {
            let stopped = lock(&saves);
            if *stopped {
                Ok(())
            } else {
                store.save(&snapshot)
            }
        })
        .await;
        let error = match saved {
            Ok(Ok(())) => continue,
            Ok(Err(e)) => e,
            Err(e) => e.to_string(),
        };
        if let Some(s) = svc.upgrade() {
            s.core.host().emit(vec![Event::new(
                event_kind(EVENT_PERSIST_FAILED),
                Level::Warn,
                json!({ "error": error }),
            )]);
        }
    }
}
