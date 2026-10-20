//! `ResidencyManager` — produkcyjny zarządca: tablica dzierżaw z kontraktu pod muteksem,
//! zegar monotoniczny, słuchacze właścicieli, kolejka zdarzeń `residency.*` dla magistrali.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use device_profile_contract::{DeviceProfile, PowerState, Profile};
use model_residency_contract::{
    Budget, Change, GpuId, GpuLayout, Grant, Lease, LeaseId, LeaseListener, LeaseRequest,
    LeaseTable, Mode, ModeSource, Placement, Residency, ResidencyClock, ResidencyError,
    ResidencyEvent, ResidencyState, Revocation,
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

/// Tożsamość karty pod danym [`GpuId`] (numer adaptera to pozycja, nie tożsamość): nazwa i LUID
/// (0 — nieznany). Służy do wykrycia, że pod tym samym numerem jest już inna karta.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CardIdentity {
    name: String,
    luid: u64,
}

impl CardIdentity {
    fn of_profile(profile: &Profile) -> BTreeMap<GpuId, Self> {
        profile
            .gpus
            .iter()
            .map(|g| {
                let identity = Self {
                    name: g.name.trim().to_owned(),
                    luid: g.luid,
                };
                (GpuId::of(g), identity)
            })
            .collect()
    }

    /// Inna karta: inna nazwa albo (gdy oba znane) inny LUID — LUID zmienia się tylko po ponownym
    /// uruchomieniu systemu albo resecie sterownika, wtedy ostrożnie traktujemy kartę jako nową.
    fn differs(&self, other: &Self) -> bool {
        self.name != other.name || (self.luid != 0 && other.luid != 0 && self.luid != other.luid)
    }
}

/// Numery kart, pod którymi w nowym profilu jest inna karta niż dotąd (zmiana kolejności
/// adapterów, np. po zmianie wyświetlacza w laptopie hybrydowym albo resecie sterownika).
fn swapped_cards(
    old: &BTreeMap<GpuId, CardIdentity>,
    new: &BTreeMap<GpuId, CardIdentity>,
) -> Vec<GpuId> {
    new.iter()
        .filter(|(id, card)| old.get(id).is_some_and(|o| o.differs(card)))
        .map(|(id, _)| *id)
        .collect()
}

/// Karty bez kart podmienionych: dzierżawy wskazujące numer podmienionej karty tracą kartę
/// ([`LeaseTable::set_hardware`] odbiera dzierżawy na kartach, których nie ma), a dzierżawy idące
/// za kartą główną przechodzą na nią w drugim kroku. Podmieniona karta główna — numer
/// zastępczy, którego nie ma żadna karta.
fn without_swapped(gpus: &GpuLayout, swapped: &[GpuId]) -> GpuLayout {
    let primary = if swapped.contains(&gpus.primary) {
        GpuId(u32::MAX)
    } else {
        gpus.primary
    };
    GpuLayout {
        primary,
        others: gpus
            .others
            .iter()
            .filter(|p| !swapped.contains(&p.gpu))
            .copied()
            .collect(),
    }
}

/// Zarządca rezydencji.
pub struct ResidencyManager {
    table: Mutex<LeaseTable>,
    /// Tożsamość kart przy ostatnim przeliczeniu pul (pusta — nieznana, np. zarządca bez profilu).
    cards: Mutex<BTreeMap<GpuId, CardIdentity>>,
    clock: Arc<dyn ResidencyClock>,
    listeners: Mutex<BTreeMap<String, Arc<dyn LeaseListener>>>,
    sink: Mutex<Option<UnboundedSender<ResidencyEvent>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl ResidencyManager {
    /// Zarządca z budżetem maszyny z jedną kartą i zegarem.
    pub fn new(budget: Budget, clock: Arc<dyn ResidencyClock>) -> Self {
        Self::with_gpus(budget, GpuLayout::default(), clock)
    }

    /// Zarządca z budżetem karty głównej i RAM, pulami pozostałych kart i zegarem.
    pub fn with_gpus(budget: Budget, gpus: GpuLayout, clock: Arc<dyn ResidencyClock>) -> Self {
        Self {
            table: Mutex::new(LeaseTable::with_gpus(budget, gpus)),
            cards: Mutex::new(BTreeMap::new()),
            clock,
            listeners: Mutex::new(BTreeMap::new()),
            sink: Mutex::new(None),
        }
    }

    /// Zarządca z budżetem z rekomendacji `device-profile` (z nakładką konfiguracji maszyny —
    /// dotyczy karty głównej i RAM), osobnymi pulami pozostałych kart z profilu
    /// ([`GpuLayout::from_profile`]) i trybem z bieżących sygnałów (pełny ekran, bateria).
    pub fn from_device(device: Arc<dyn DeviceProfile>, config: &ResidencyConfig) -> Self {
        let (budget, gpus) = config.hardware(device.as_ref());
        if !gpus.others.is_empty() {
            tracing::info!(główna = %gpus.primary, pozostałe = ?gpus.others, "osobne pule pamięci kart");
        }
        let manager = Self::with_gpus(budget, gpus, Arc::new(MonotonicClock::new()));
        *lock(&manager.cards) = CardIdentity::of_profile(&device.current());
        manager.refresh_mode(&config.signals(DeviceSignals(device)));
        manager
    }

    /// Zmiana sprzętu (C6.6): budżet karty głównej i RAM oraz karty z profilu urządzenia; gdy
    /// różnią się od obowiązujących — [`Residency::set_hardware`] (pule przeliczone, dzierżawy na
    /// kartach, których już nie ma, odebrane). Numer karty to pozycja adaptera, więc gdy pod tym
    /// samym numerem jest już inna karta (inna nazwa albo LUID), dzierżawy wskazujące ten numer są
    /// odbierane, a nie przenoszone na obcą kartę. `None` — sprzęt bez zmian (nic nie robi).
    pub fn sync_hardware(
        &self,
        device: &dyn DeviceProfile,
        config: &ResidencyConfig,
    ) -> Option<Vec<Change>> {
        let (budget, gpus) = config.hardware(device);
        let gpus = gpus.normalized();
        let identities = CardIdentity::of_profile(&device.current());
        let swapped = {
            let mut cards = lock(&self.cards);
            let swapped = swapped_cards(&cards, &identities);
            *cards = identities;
            swapped
        };
        let changes = {
            let mut t = self.table();
            if swapped.is_empty() && t.base_budget() == budget && *t.gpus() == gpus {
                return None;
            }
            tracing::info!(?budget, główna = %gpus.primary, pozostałe = ?gpus.others,
                podmienione = ?swapped, "zmiana sprzętu: nowe pule pamięci kart");
            let mut changes = if swapped.is_empty() {
                Vec::new()
            } else {
                t.set_hardware(budget, without_swapped(&gpus, &swapped))
            };
            changes.extend(t.set_hardware(budget, gpus));
            changes
        };
        self.notify(&changes);
        Some(changes)
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
                    gpu = ?grant.lease.gpu, evicted = grant.evicted.len(), "przyznano dzierżawę");
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

    fn set_gpus(&self, gpus: GpuLayout) -> Vec<Change> {
        let changes = self.table().set_gpus(gpus);
        self.notify(&changes);
        changes
    }

    fn set_hardware(&self, budget: Budget, gpus: GpuLayout) -> Vec<Change> {
        let changes = self.table().set_hardware(budget, gpus);
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
