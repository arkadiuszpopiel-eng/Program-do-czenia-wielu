//! Atrapa pełnego `scheduler` (SPEC „Fake”): ten sam deterministyczny rdzeń co `-impl`
//! ([`SchedCore`]), ale na **wirtualnym zegarze** (czas płynie tylko przez [`FakeScheduler::advance`]),
//! ze skryptowanymi wykonawczyniami ([`Script`]), zdarzeniami nagrywanymi zamiast magistrali,
//! stanem w pamięci (restart = nowy rdzeń ze stanu) i ustawialną decyzją budżetu tła.
//! Do testów `agent-runtime`, `triggers`, `marshal`, UI — wyłącznie jako dev-dependency.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod sim;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::Event;
use cost_meter_contract::BudgetDecision;
use scheduler_contract::contract_tests::Script;
use scheduler_contract::{
    Dispatch, DispatchId, Holder, Lease, LeaseInfo, LeaseRequest, MemSnapshotStore, PreemptReason,
    QueuedRequest, Resource, Roster, SchedCore, SchedError, SchedHost, Scheduler, SchedulerLite,
    Snapshot, SnapshotStore, Steer, SteerEnvelope, SystemConditions, TaskError, TaskId, TaskSpec,
    TaskView, Termination,
};

use crate::sim::Sim;

/// Początek wirtualnego czasu (ms od epoki UTC; wrzesień 2026).
pub const EPOCH_MS: u64 = 1_790_000_000_000;

/// Ile razy z rzędu wolno przetwarzać tę samą chwilę (ochrona przed pętlą bez postępu).
const SAME_INSTANT_LIMIT: usize = 10_000;

/// Otoczenie atrapy: wirtualny zegar, nagranie zdarzeń, magazyn stanu, budżet tła.
pub struct FakeHost {
    clock: AtomicU64,
    events: Mutex<Vec<Event>>,
    store: MemSnapshotStore,
    budget: Mutex<BudgetDecision>,
}

impl Default for FakeHost {
    fn default() -> Self {
        Self {
            clock: AtomicU64::new(EPOCH_MS),
            events: Mutex::new(Vec::new()),
            store: MemSnapshotStore::new(),
            budget: Mutex::new(BudgetDecision::Allow),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl SchedHost for FakeHost {
    fn now_ms(&self) -> u64 {
        self.clock.load(Ordering::SeqCst)
    }

    fn emit(&self, events: Vec<Event>) {
        lock(&self.events).extend(events);
    }

    fn persist(&self, snapshot: &Snapshot) {
        // Magazyn w pamięci nie zawodzi.
        let _ = self.store.save(snapshot);
    }

    fn check_background_budget(&self, _estimate_micro_pln: u64) -> BudgetDecision {
        lock(&self.budget).clone()
    }
}

impl FakeHost {
    fn set_now(&self, ms: u64) {
        self.clock.store(ms, Ordering::SeqCst);
    }
}

/// Scheduler-atrapa.
pub struct FakeScheduler {
    host: Arc<FakeHost>,
    core: Mutex<Arc<SchedCore<FakeHost>>>,
    sim: Mutex<Sim>,
}

impl Default for FakeScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeScheduler {
    /// Nowa atrapa (czas [`EPOCH_MS`], obsada „Standard”).
    pub fn new() -> Self {
        let host = Arc::new(FakeHost::default());
        Self {
            core: Mutex::new(SchedCore::new(Arc::clone(&host))),
            host,
            sim: Mutex::new(Sim::default()),
        }
    }

    /// Nowa atrapa z zegarem ustawionym na `start_ms` (ms UTC).
    pub fn starting_at(start_ms: u64) -> Self {
        let f = Self::new();
        f.host.set_now(start_ms);
        f
    }

    /// Rdzeń (bieżący — po restarcie nowy).
    pub fn core(&self) -> Arc<SchedCore<FakeHost>> {
        Arc::clone(&lock(&self.core))
    }

    /// Bieżący czas wirtualny (ms).
    pub fn now_ms(&self) -> u64 {
        self.host.now_ms()
    }

    /// Skrypt wykonawczyni zadania (domyślnie: 1 krok, 10 ms, sukces).
    pub fn script(&self, task: &TaskId, script: Script) {
        lock(&self.sim).scripts.insert(task.clone(), script);
    }

    /// Decyzja budżetu tła zwracana przez otoczenie (`cost-meter`).
    pub fn set_background_budget(&self, decision: BudgetDecision) {
        *lock(&self.host.budget) = decision;
    }

    /// Nagrane zdarzenia (to, co `-impl` opublikowałby na magistrali).
    pub fn events(&self) -> Vec<Event> {
        lock(&self.host.events).clone()
    }

    /// Nagrane zdarzenia o nazwie.
    pub fn events_named(&self, name: &str) -> Vec<Event> {
        self.events()
            .into_iter()
            .filter(|e| e.kind.as_str() == name)
            .collect()
    }

    /// Steering widziany przez wykonawczynię zadania: (krok, przed którym dostarczono, wiadomość).
    pub fn seen_steering(&self, task: &TaskId) -> Vec<(u32, SteerEnvelope)> {
        lock(&self.sim).seen.get(task).cloned().unwrap_or_default()
    }

    /// Wszystkie wysłania (także sprzed restartu).
    pub fn dispatches(&self) -> Vec<Dispatch> {
        lock(&self.sim).dispatches.clone()
    }

    /// Wysłania przerwane siłą.
    pub fn aborted(&self) -> Vec<DispatchId> {
        lock(&self.sim).aborted.clone()
    }

    /// Zasoby trzymane teraz przez zadania.
    pub fn held(&self) -> BTreeMap<TaskId, Vec<Resource>> {
        self.core().held_resources()
    }

    /// Ostatni zapisany stan.
    pub fn stored_snapshot(&self) -> Option<Snapshot> {
        self.host.store.load().ok().flatten()
    }

    /// Restart procesu: wykonawczynie i dzierżawy giną, nowy rdzeń z zapisanego stanu.
    pub fn restart(&self) -> Result<(), String> {
        let snapshot = match self.stored_snapshot() {
            Some(s) => s,
            None => self.core().snapshot(),
        };
        lock(&self.sim).workers.clear();
        let fresh = SchedCore::restore(Arc::clone(&self.host), snapshot)?;
        *lock(&self.core) = fresh;
        self.settle();
        Ok(())
    }

    /// Najbliższa chwila zdarzenia (krok wykonawczyni, termin rdzenia lub `scheduler-lite`).
    pub fn next_event_at(&self) -> Option<u64> {
        let worker = lock(&self.sim).next_step_at();
        let core = self.core().next_wake();
        match (worker, core) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Przesuwa czas o `ms`, obsługując po kolei każde zdarzenie po drodze.
    pub fn advance(&self, ms: u64) {
        let target = self.now_ms().saturating_add(ms);
        let mut last = None;
        let mut same = 0usize;
        self.settle();
        while let Some(t) = self.next_event_at().filter(|t| *t <= target) {
            let t = t.max(self.now_ms());
            if last == Some(t) {
                same += 1;
                if same > SAME_INSTANT_LIMIT {
                    break;
                }
            } else {
                same = 0;
                last = Some(t);
            }
            self.host.set_now(t);
            self.step_workers(t);
            self.settle();
        }
        self.host.set_now(target);
        self.settle();
    }

    /// Przegląd rdzenia i `scheduler-lite` w bieżącej chwili.
    fn settle(&self) {
        let core = self.core();
        core.locks().tick();
        let effects = core.pump();
        lock(&self.sim).apply(effects, self.now_ms());
    }

    /// Kończy kroki wykonawczyń przypadające na `now` (kolejność wysłań).
    fn step_workers(&self, now: u64) {
        let core = self.core();
        let due = lock(&self.sim).due(now);
        for dispatch in due {
            let Some(mut worker) = lock(&self.sim).workers.remove(&dispatch) else {
                continue;
            };
            let result = match worker.run.complete_step() {
                Ok(report) => {
                    let (directive, effects) = core.boundary(dispatch, &report);
                    let end = worker.run.directive(directive);
                    let mut sim = lock(&self.sim);
                    sim.record(&mut worker);
                    sim.apply(effects, now);
                    match end {
                        None => {
                            worker.next_at = worker.run.step_duration().map(|ms| now + ms);
                            sim.workers.insert(dispatch, worker);
                            continue;
                        }
                        Some(result) => result,
                    }
                }
                Err(result) => result,
            };
            let effects = core.finish(dispatch, result);
            lock(&self.sim).apply(effects, now);
        }
    }

    /// Delegacja z wykonania (to, co wykonawczyni robi przez `StepGate::spawn`).
    pub fn spawn(
        &self,
        dispatch: DispatchId,
        tasks: Vec<TaskSpec>,
    ) -> Result<Vec<TaskId>, TaskError> {
        self.wrap(self.core().spawn(dispatch, tasks))
    }

    fn wrap<T>(
        &self,
        r: Result<(T, Vec<scheduler_contract::SchedEffect>), TaskError>,
    ) -> Result<T, TaskError> {
        let (value, effects) = r?;
        lock(&self.sim).apply(effects, self.now_ms());
        Ok(value)
    }
}

#[async_trait]
impl SchedulerLite for FakeScheduler {
    async fn acquire(&self, request: LeaseRequest) -> Result<Lease, SchedError> {
        let locks = Arc::clone(self.core().locks());
        locks.acquire(request).await
    }

    fn preempt(
        &self,
        resource: &Resource,
        by: Holder,
        reason: PreemptReason,
    ) -> Result<(), SchedError> {
        self.core().locks().preempt(resource, by, reason)
    }

    fn handoff(&self, resource: &Resource, from: &Holder, to: Holder) -> Result<(), SchedError> {
        self.core().locks().handoff_from(resource, from, to)
    }

    fn holder(&self, resource: &Resource) -> Option<LeaseInfo> {
        self.core().locks().holder(resource)
    }

    fn queue(&self, resource: &Resource) -> Vec<QueuedRequest> {
        self.core().locks().queue(resource)
    }

    fn kill_all(&self) -> usize {
        let (n, effects) = self.core().kill_all();
        lock(&self.sim).apply(effects, self.now_ms());
        n
    }
}

#[async_trait]
impl Scheduler for FakeScheduler {
    fn submit(&self, tasks: Vec<TaskSpec>) -> Result<Vec<TaskId>, TaskError> {
        self.wrap(self.core().submit(tasks))
    }

    fn cancel(&self, task: &TaskId, reason: &str) -> Result<Vec<TaskId>, TaskError> {
        self.wrap(self.core().cancel(task, reason))
    }

    fn steer(&self, task: &TaskId, steer: Steer) -> Result<u64, TaskError> {
        self.wrap(self.core().steer(task, steer))
    }

    fn pause(&self, task: &TaskId) -> Result<(), TaskError> {
        self.wrap(self.core().pause(task).map(|fx| ((), fx)))
    }

    fn resume(&self, task: &TaskId) -> Result<(), TaskError> {
        self.wrap(self.core().resume(task).map(|fx| ((), fx)))
    }

    fn task(&self, task: &TaskId) -> Option<TaskView> {
        self.core().task(task)
    }

    fn tasks(&self) -> Vec<TaskView> {
        self.core().tasks()
    }

    fn set_roster(&self, roster: Roster) {
        let effects = self.core().set_roster(roster);
        lock(&self.sim).apply(effects, self.now_ms());
    }

    fn set_conditions(&self, conditions: SystemConditions) {
        let effects = self.core().set_conditions(conditions);
        lock(&self.sim).apply(effects, self.now_ms());
    }

    /// Atrapa sama przesuwa wirtualny czas do zakończenia zadania (każde zadanie ma termin).
    async fn wait(&self, task: &TaskId) -> Result<Termination, TaskError> {
        loop {
            let view = self
                .core()
                .task(task)
                .ok_or_else(|| TaskError::UnknownTask(task.clone()))?;
            if let Some(t) = view.state.termination() {
                return Ok(t.clone());
            }
            let next = self
                .next_event_at()
                .ok_or_else(|| TaskError::UnknownTask(task.clone()))?;
            self.advance(next.saturating_sub(self.now_ms()).max(1));
        }
    }
}
