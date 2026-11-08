//! Implementacja `scheduler-lite` (docs/modules/scheduler-lite/SPEC.md, PLAN §9.3).
//!
//! Rdzeń decyzyjny (`Core`/`LockTable`) pochodzi z kontraktu; ten crate dostarcza zegar tokio
//! (monotoniczny, ms od startu), zadanie timera (timeouty, wygasanie rezerwacji przekazania),
//! publikację zdarzeń `scheduler.lease.*` na magistralę i cykl życia modułu. Brak I/O.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use scheduler_lite_contract::{
    Core, Holder, Host, Lease, LeaseInfo, LeaseRequest, PreemptReason, QueuedRequest, Resource,
    ResourcePolicy, SchedError, SchedulerLite, millis,
};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Otoczenie produkcyjne: zegar tokio i kanał do zadania publikującego na magistralę.
pub struct BusHost {
    start: Instant,
    events: mpsc::UnboundedSender<Vec<Event>>,
    wake: Arc<Notify>,
}

impl Host for BusHost {
    fn now_ms(&self) -> u64 {
        millis(self.start.elapsed())
    }

    fn emit(&self, events: Vec<Event>) {
        // Zamknięty kanał = moduł zatrzymany; zdarzenia diagnostyczne można pominąć.
        let _ = self.events.send(events);
    }

    fn deadline_changed(&self) {
        self.wake.notify_one();
    }
}

struct Running {
    core: Arc<Core<BusHost>>,
    timer: JoinHandle<()>,
}

/// Moduł schedulera zasobów wyłącznych.
pub struct SchedulerModule {
    manifest: ModuleManifest,
    policies: Mutex<Vec<(Resource, ResourcePolicy)>>,
    running: Mutex<Option<Running>>,
}

impl SchedulerModule {
    /// Nowy (niewystartowany) moduł.
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            policies: Mutex::new(Vec::new()),
            running: Mutex::new(None),
        })
    }

    /// Nadpisuje politykę zasobu (np. z `[scheduler]` w konfiguracji); działa też po starcie.
    pub fn set_policy(&self, resource: Resource, policy: ResourcePolicy) {
        lock(&self.policies).push((resource.clone(), policy));
        if let Some(running) = lock(&self.running).as_ref() {
            running.core.set_policy(resource, policy);
        }
    }

    fn core(&self) -> Result<Arc<Core<BusHost>>, SchedError> {
        lock(&self.running)
            .as_ref()
            .map(|r| Arc::clone(&r.core))
            .ok_or(SchedError::NotStarted)
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Zadanie timera: śpi do najbliższego terminu albo do zmiany terminów, potem `tick`.
async fn timer(core: Weak<Core<BusHost>>, wake: Arc<Notify>, start: Instant) {
    loop {
        let Some(next) = core.upgrade().map(|c| c.next_deadline()) else {
            return;
        };
        match next {
            Some(ms) => {
                let at = start + Duration::from_millis(ms);
                tokio::select! {
                    () = tokio::time::sleep_until(at) => match core.upgrade() {
                        Some(c) => c.tick(),
                        None => return,
                    },
                    () = wake.notified() => {}
                }
            }
            None => wake.notified().await,
        }
    }
}

/// Zadanie publikujące zdarzenia na magistralę (publikacja jest asynchroniczna, decyzje nie).
async fn publisher(bus: Arc<dyn EventBus>, mut rx: mpsc::UnboundedReceiver<Vec<Event>>) {
    while let Some(events) = rx.recv().await {
        for event in events {
            // Błąd magistrali nie cofa decyzji (zdarzenia są diagnostyczne).
            let _ = bus.publish(event).await;
        }
    }
}

#[async_trait]
impl SchedulerLite for SchedulerModule {
    async fn acquire(&self, request: LeaseRequest) -> Result<Lease, SchedError> {
        let core = self.core()?;
        core.acquire(request).await
    }

    fn preempt(
        &self,
        resource: &Resource,
        by: Holder,
        reason: PreemptReason,
    ) -> Result<(), SchedError> {
        self.core()?.preempt(resource, by, reason)
    }

    fn handoff(&self, resource: &Resource, from: &Holder, to: Holder) -> Result<(), SchedError> {
        self.core()?.handoff_from(resource, from, to)
    }

    fn holder(&self, resource: &Resource) -> Option<LeaseInfo> {
        self.core().ok()?.holder(resource)
    }

    fn queue(&self, resource: &Resource) -> Vec<QueuedRequest> {
        self.core().map(|c| c.queue(resource)).unwrap_or_default()
    }

    fn kill_all(&self) -> usize {
        self.core().map(|c| c.kill_all()).unwrap_or(0)
    }
}

#[async_trait]
impl Module for SchedulerModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut running = lock(&self.running);
        if running.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        let (tx, rx) = mpsc::unbounded_channel();
        let wake = Arc::new(Notify::new());
        let start = Instant::now();
        let core = Core::new(BusHost {
            start,
            events: tx,
            wake: Arc::clone(&wake),
        });
        for (resource, policy) in lock(&self.policies).iter() {
            core.set_policy(resource.clone(), *policy);
        }
        let timer = tokio::spawn(timer(Arc::downgrade(&core), wake, start));
        // Publikator kończy się sam, gdy rdzeń (nadawca kanału) zniknie — po opróżnieniu kolejki.
        drop(tokio::spawn(publisher(ctx.bus, rx)));
        *running = Some(Running { core, timer });
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        let running = lock(&self.running).take().ok_or(ModuleError::NotStarted)?;
        // Zatrzymanie = kill-switch dla tego modułu: dzierżawy odebrane, czekające anulowane.
        running.core.kill_all();
        running.timer.abort();
        drop(running.core);
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        match self.running.try_lock() {
            Ok(guard) if guard.is_some() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::NotStarted,
            Err(_) => HealthStatus::Degraded("stan zajęty".into()),
        }
    }
}
