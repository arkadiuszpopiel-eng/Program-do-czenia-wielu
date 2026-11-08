//! Atrapa `scheduler-lite` (SPEC „Fake”): ten sam deterministyczny rdzeń co `-impl`, ale na
//! **wirtualnym zegarze** (czas płynie tylko przez `advance`), ze zdarzeniami nagrywanymi zamiast
//! magistrali, zapisem żądań i wstrzykiwaniem błędów. Do testów `voice-dialog`, `voice-tts`,
//! `agent-runtime` i innych — wyłącznie jako dev-dependency.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::Event;
use scheduler_lite_contract::{
    Core, Holder, Host, Lease, LeaseInfo, LeaseRequest, LockTable, PreemptReason, QueuedRequest,
    Resource, ResourcePolicy, SchedError, SchedulerLite,
};

/// Wirtualny zegar w milisekundach (klonowanie dzieli stan).
#[derive(Debug, Clone, Default)]
pub struct VirtualClock(Arc<AtomicU64>);

impl VirtualClock {
    /// Bieżący czas (ms).
    pub fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    fn set(&self, ms: u64) {
        self.0.store(ms, Ordering::SeqCst);
    }
}

/// Otoczenie atrapy: wirtualny zegar + nagranie zdarzeń.
#[derive(Default)]
pub struct FakeHost {
    clock: VirtualClock,
    events: Mutex<Vec<Event>>,
}

impl Host for FakeHost {
    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    fn emit(&self, events: Vec<Event>) {
        lock(&self.events).extend(events);
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Scheduler-atrapa.
pub struct FakeScheduler {
    core: Arc<Core<FakeHost>>,
    requests: Mutex<Vec<LeaseRequest>>,
    fail_next: Mutex<Option<SchedError>>,
}

impl Default for FakeScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeScheduler {
    /// Nowa atrapa (czas 0 ms, polityki domyślne).
    pub fn new() -> Self {
        Self {
            core: Core::new(FakeHost::default()),
            requests: Mutex::new(Vec::new()),
            fail_next: Mutex::new(None),
        }
    }

    /// Wirtualny zegar.
    pub fn clock(&self) -> &VirtualClock {
        &self.core.host().clock
    }

    /// Przesuwa czas o `ms`, obsługując po kolei każdy termin po drodze (timeouty, rezerwacje).
    pub fn advance(&self, ms: u64) {
        let target = self.clock().now_ms().saturating_add(ms);
        while let Some(deadline) = self.core.next_deadline().filter(|d| *d <= target) {
            self.clock().set(deadline.max(self.clock().now_ms()));
            self.core.tick();
        }
        self.clock().set(target);
    }

    /// Zdarzenia, które `-impl` opublikowałby na magistrali.
    pub fn events(&self) -> Vec<Event> {
        lock(&self.core.host().events).clone()
    }

    /// Wszystkie żądania `acquire` (także odrzucone).
    pub fn requests(&self) -> Vec<LeaseRequest> {
        lock(&self.requests).clone()
    }

    /// Następne `acquire` zwróci ten błąd (jednorazowo).
    pub fn fail_next(&self, error: SchedError) {
        *lock(&self.fail_next) = Some(error);
    }

    /// Nadpisuje politykę zasobu.
    pub fn set_policy(&self, resource: Resource, policy: ResourcePolicy) {
        self.core.set_policy(resource, policy);
    }

    /// Kopia tablicy blokad (asercje w testach).
    pub fn snapshot(&self) -> LockTable {
        self.core.snapshot()
    }
}

#[async_trait]
impl SchedulerLite for FakeScheduler {
    async fn acquire(&self, request: LeaseRequest) -> Result<Lease, SchedError> {
        lock(&self.requests).push(request.clone());
        let injected = lock(&self.fail_next).take();
        if let Some(err) = injected {
            return Err(err);
        }
        self.core.acquire(request).await
    }

    fn preempt(
        &self,
        resource: &Resource,
        by: Holder,
        reason: PreemptReason,
    ) -> Result<(), SchedError> {
        self.core.preempt(resource, by, reason)
    }

    fn handoff(&self, resource: &Resource, from: &Holder, to: Holder) -> Result<(), SchedError> {
        self.core.handoff_from(resource, from, to)
    }

    fn holder(&self, resource: &Resource) -> Option<LeaseInfo> {
        self.core.holder(resource)
    }

    fn queue(&self, resource: &Resource) -> Vec<QueuedRequest> {
        self.core.queue(resource)
    }

    fn kill_all(&self) -> usize {
        self.core.kill_all()
    }
}
