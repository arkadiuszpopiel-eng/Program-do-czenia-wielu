//! `ResidencyManager` — produkcyjny zarządca: tablica dzierżaw z kontraktu pod muteksem,
//! zegar monotoniczny, słuchacze właścicieli, kolejka zdarzeń `residency.*` dla magistrali.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use device_profile_contract::{DeviceProfile, PowerState};
use model_residency_contract::{
    Budget, Change, Grant, Lease, LeaseId, LeaseListener, LeaseRequest, LeaseTable, Mode,
    ModeSource, Placement, Residency, ResidencyClock, ResidencyError, ResidencyEvent,
    ResidencyState, Revocation,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::ResidencyConfig;

/// Zegar monotoniczny (ms od utworzenia).
#[derive(Debug)]
pub struct MonotonicClock(Instant);

impl MonotonicClock {
    /// Zegar od teraz.
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl ResidencyClock for MonotonicClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Sygnały trybu z `device-profile` (pełny ekran, bateria).
pub struct DeviceSignals(pub Arc<dyn DeviceProfile>);

impl ModeSource for DeviceSignals {
    fn fullscreen_active(&self) -> bool {
        self.0.fullscreen_active()
    }

    fn on_battery(&self) -> bool {
        matches!(self.0.power_state(), PowerState::Battery { .. })
    }
}

/// Zarządca rezydencji.
pub struct ResidencyManager {
    table: Mutex<LeaseTable>,
    clock: Arc<dyn ResidencyClock>,
    listeners: Mutex<BTreeMap<String, Arc<dyn LeaseListener>>>,
    sink: Mutex<Option<UnboundedSender<ResidencyEvent>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl ResidencyManager {
    /// Zarządca z budżetem i zegarem.
    pub fn new(budget: Budget, clock: Arc<dyn ResidencyClock>) -> Self {
        Self {
            table: Mutex::new(LeaseTable::new(budget)),
            clock,
            listeners: Mutex::new(BTreeMap::new()),
            sink: Mutex::new(None),
        }
    }

    /// Zarządca z budżetem z rekomendacji `device-profile` (z nakładką konfiguracji maszyny)
    /// i trybem z bieżących sygnałów (pełny ekran, bateria).
    pub fn from_device(device: Arc<dyn DeviceProfile>, config: &ResidencyConfig) -> Self {
        let budget = config.budget(&device.recommend().residency);
        let manager = Self::new(budget, Arc::new(MonotonicClock::new()));
        manager.refresh_mode(&config.signals(DeviceSignals(device)));
        manager
    }

    /// Podłącza kolejkę zdarzeń (moduł przekazuje je na magistralę w kolejności).
    pub fn set_event_sink(&self, sink: Option<UnboundedSender<ResidencyEvent>>) {
        *lock(&self.sink) = sink;
    }

    fn emit(&self, events: Vec<ResidencyEvent>) {
        if let Some(tx) = lock(&self.sink).as_ref() {
            for ev in events {
                // Odbiorca zamknięty (moduł zatrzymany) — zdarzenia diagnostyczne można pominąć.
                let _ = tx.send(ev);
            }
        }
    }

    fn notify(&self, changes: &[Change]) {
        let listeners = lock(&self.listeners).clone();
        for change in changes {
            match change {
                Change::Revoked(r) => {
                    if let Some(l) = listeners.get(&r.lease.request.owner) {
                        l.revoked(r);
                    }
                }
                Change::Moved(lease) => {
                    if let Some(l) = listeners.get(&lease.request.owner) {
                        l.moved(lease);
                    }
                }
            }
        }
        self.emit(ResidencyEvent::from_changes(changes));
    }

    fn table(&self) -> MutexGuard<'_, LeaseTable> {
        lock(&self.table)
    }
}

impl Residency for ResidencyManager {
    fn acquire(&self, request: LeaseRequest) -> Result<Grant, ResidencyError> {
        let now = self.clock.now_ms();
        let result = self.table().acquire(&request, now);
        match &result {
            Ok(grant) => {
                let revoked: Vec<Change> =
                    grant.evicted.iter().cloned().map(Change::Revoked).collect();
                let listeners = lock(&self.listeners).clone();
                for change in &revoked {
                    if let Change::Revoked(r) = change
                        && let Some(l) = listeners.get(&r.lease.request.owner)
                    {
                        l.revoked(r);
                    }
                }
                let preferred_gpu = request.placement != Placement::CpuOnly;
                self.emit(ResidencyEvent::from_grant(grant, preferred_gpu));
                tracing::debug!(model = %grant.lease.request.model, device = ?grant.lease.device,
                    evicted = grant.evicted.len(), "przyznano dzierżawę");
            }
            Err(ResidencyError::TooLarge {
                model,
                vram_mb,
                ram_mb,
            }) => self.emit(vec![ResidencyEvent::BudgetExceeded {
                model: model.clone(),
                vram_mb: *vram_mb,
                ram_mb: *ram_mb,
            }]),
            Err(_) => {}
        }
        result
    }

    fn release(&self, id: LeaseId) -> Result<(), ResidencyError> {
        let lease = self.table().release(id)?;
        self.emit(vec![ResidencyEvent::Released { lease }]);
        Ok(())
    }

    fn touch(&self, id: LeaseId) -> Result<(), ResidencyError> {
        let now = self.clock.now_ms();
        self.table().touch(id, now)
    }

    fn set_in_use(&self, id: LeaseId, in_use: bool) -> Result<(), ResidencyError> {
        let now = self.clock.now_ms();
        self.table().set_in_use(id, in_use, now)
    }

    fn lease(&self, id: LeaseId) -> Option<Lease> {
        self.table().lease(id).cloned()
    }

    fn snapshot(&self) -> ResidencyState {
        self.table().snapshot()
    }

    fn set_mode(&self, mode: Mode) -> Vec<Change> {
        let (from, changes) = {
            let mut t = self.table();
            let from = t.mode();
            (from, t.set_mode(mode))
        };
        if from != mode {
            tracing::info!(?from, to = ?mode, "zmiana trybu rezydencji");
            self.emit(vec![ResidencyEvent::ModeChanged { from, to: mode }]);
        }
        self.notify(&changes);
        changes
    }

    fn set_budget(&self, budget: Budget) -> Vec<Change> {
        let changes = self.table().set_budget(budget);
        self.notify(&changes);
        changes
    }

    fn reap_idle(&self) -> Vec<Revocation> {
        let now = self.clock.now_ms();
        let reaped = self.table().reap_idle(now);
        let changes: Vec<Change> = reaped.iter().cloned().map(Change::Revoked).collect();
        self.notify(&changes);
        reaped
    }

    fn listen(&self, owner: &str, listener: Arc<dyn LeaseListener>) {
        lock(&self.listeners).insert(owner.to_owned(), listener);
    }
}
