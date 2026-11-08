//! Atrapa rejestru modułów (docs/PLAN.md §4.5, SPEC core-registry „Fake”).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod ops;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{
    ContractRef, EVENT_STATE_CHANGED, HealthStatus, Module, ModuleContext, ModuleError, ModuleId,
    ModuleManifest, ModuleState, RegistryError, registry_event_kind,
};
use serde_json::json;

/// Domyślny limit bezczynności atrapy (jak w implementacji: 10 min).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(600);

/// Wirtualny zegar atrapy (czas od utworzenia); klony dzielą stan.
#[derive(Debug, Clone, Default)]
pub struct FakeClock(Arc<Mutex<Duration>>);

impl FakeClock {
    /// Przesuwa czas do przodu.
    pub fn advance(&self, by: Duration) {
        let mut now = lock(&self.0);
        *now = now.saturating_add(by);
    }

    /// Czas od utworzenia.
    pub fn elapsed(&self) -> Duration {
        *lock(&self.0)
    }
}

/// Moduł-wydmuszka dla `register_manifest` (start/stop zawsze udane).
struct NoopModule {
    manifest: ModuleManifest,
    running: bool,
}

#[async_trait]
impl Module for NoopModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, _ctx: ModuleContext) -> Result<(), ModuleError> {
        self.running = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.running = false;
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        if self.running {
            HealthStatus::Healthy
        } else {
            HealthStatus::NotStarted
        }
    }
}

pub(crate) struct Slot {
    pub module: Box<dyn Module>,
    pub manifest: ModuleManifest,
    pub state: ModuleState,
    pub last_used: Duration,
    pub failures: u8,
    pub fail_next_start: bool,
}

/// Rejestr w pamięci z wirtualnym czasem; implementuje `Registry` z kontraktu.
pub struct FakeRegistry {
    pub(crate) slots: tokio::sync::Mutex<BTreeMap<ModuleId, Slot>>,
    pub(crate) bus: Arc<dyn EventBus>,
    pub(crate) clock: FakeClock,
    pub(crate) calls: Mutex<Vec<String>>,
    pub(crate) idle_timeout: Duration,
    pub(crate) external: BTreeSet<ContractRef>,
}

impl FakeRegistry {
    /// Atrapa publikująca zdarzenia na `bus`; kontrakty zewnętrzne: `core-bus-contract@1`.
    pub fn new(bus: Arc<dyn EventBus>) -> Self {
        let external = [ContractRef {
            name: "core-bus-contract".into(),
            major: 1,
        }]
        .into();
        Self {
            slots: tokio::sync::Mutex::new(BTreeMap::new()),
            bus,
            clock: FakeClock::default(),
            calls: Mutex::new(Vec::new()),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            external,
        }
    }

    /// Ustawia limit bezczynności (builder).
    #[must_use]
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Ustawia kontrakty dostarczane poza rejestrem (builder).
    #[must_use]
    pub fn with_external(mut self, contracts: impl IntoIterator<Item = ContractRef>) -> Self {
        self.external = contracts.into_iter().collect();
        self
    }

    /// Limit bezczynności.
    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// Przesuwa wirtualny czas atrapy.
    pub fn advance(&self, by: Duration) {
        self.clock.advance(by);
    }

    /// Wirtualny czas od utworzenia atrapy.
    pub fn elapsed(&self) -> Duration {
        self.clock.elapsed()
    }

    /// Uchwyt zegara (do przesuwania czasu, gdy rejestr jest już przeniesiony).
    pub fn clock(&self) -> FakeClock {
        self.clock.clone()
    }

    /// Dziennik wywołań traitu (`"acquire:x-contract@1"`, `"activate:x"`, …).
    pub fn calls(&self) -> Vec<String> {
        lock(&self.calls).clone()
    }

    /// Rejestruje moduł-wydmuszkę z samego manifestu (fixture).
    pub async fn register_manifest(&self, manifest: ModuleManifest) -> Result<(), RegistryError> {
        let module = NoopModule {
            manifest,
            running: false,
        };
        core_registry_contract::Registry::register(self, Box::new(module)).await
    }

    /// Wymusza stan (symulacja `Failed`/`Degraded`); publikuje zmianę stanu.
    pub async fn set_state(&self, id: &ModuleId, state: ModuleState) -> Result<(), RegistryError> {
        let mut slots = self.slots.lock().await;
        if !slots.contains_key(id) {
            return Err(RegistryError::UnknownModule(id.clone()));
        }
        self.transition(&mut slots, id, state).await;
        Ok(())
    }

    /// Następny start modułu zakończy się błędem (jednorazowo).
    pub async fn fail_next_start(&self, id: &ModuleId) -> Result<(), RegistryError> {
        let mut slots = self.slots.lock().await;
        let slot = slots
            .get_mut(id)
            .ok_or_else(|| RegistryError::UnknownModule(id.clone()))?;
        slot.fail_next_start = true;
        Ok(())
    }

    pub(crate) fn note(&self, call: String) {
        lock(&self.calls).push(call);
    }

    pub(crate) async fn transition(
        &self,
        slots: &mut BTreeMap<ModuleId, Slot>,
        id: &ModuleId,
        to: ModuleState,
    ) {
        let Some(slot) = slots.get_mut(id) else {
            return;
        };
        if slot.state == to {
            return;
        }
        let from = std::mem::replace(&mut slot.state, to.clone());
        let payload = json!({"module": id.as_str(), "from": from.name(), "to": to.name()});
        let event = Event::new(
            registry_event_kind(EVENT_STATE_CHANGED),
            Level::Info,
            payload,
        );
        // Atrapa: błąd magistrali nie zmienia wyniku operacji (jak w implementacji).
        let _ = self.bus.publish(event).await;
    }
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}
