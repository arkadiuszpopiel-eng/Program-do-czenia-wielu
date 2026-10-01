//! Kontrakt zarządcy rezydencji modeli w RAM/VRAM (docs/modules/model-residency/SPEC.md,
//! PLAN §3.4, §3.5, §6.3).
//!
//! - dzierżawy ([`Lease`]) dla STT/TTS/LLM/embeddera/VAD z szacowanym zużyciem;
//! - budżet z `device-profile` ([`Budget::from_device`]; baseline 8 GB: pulpit 0,5–1 GB rezerwy);
//! - „STT+TTS+LLM nie zawsze naraz": wymiana **LRU z priorytetami** (głos > rozmowa > tło),
//!   wykluczenie STT/ciężki TTS na GPU przy ciasnym VRAM;
//! - tryb gry (pełny ekran, sygnał z zewnątrz — [`ModeSource`]) → przeniesienie na CPU albo
//!   zwolnienie; tryb baterii → bez modeli tła; emulacja baseline;
//! - zdarzenia `residency.*` ([`ResidencyEvent`]).
//!
//! Reguły (planowanie, eksmisje, tryby) są czystą maszyną stanów [`LeaseTable`] — `-impl` i
//! `-fake` nie mogą się rozjechać. Niezmienniki (testy własności): suma dzierżaw ≤ budżet po
//! każdej operacji; żądanie o wyższym priorytecie nigdy nie czeka na dzierżawę o niższym.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod events;
mod select;
mod table;
mod types;

use std::sync::Arc;

pub use events::{
    EVENT_BUDGET_EXCEEDED, EVENT_EVICTED, EVENT_GRANTED, EVENT_MODE_CHANGED, EVENT_MOVED,
    EVENT_OOM_AVOIDED, EVENT_RELEASED, ResidencyEvent, event_kind,
};
pub use select::{Plan, exclusive_conflicts, fits, plan, shrink_victims, total, yields_to};
pub use table::{Change, LeaseTable};
pub use types::{
    Budget, Device, Grant, Lease, LeaseId, LeaseRequest, Mode, ModelRole, Placement, Priority,
    ResidencyError, ResidencyState, Revocation, RevokeReason, Usage,
};

/// Monotoniczny zegar zarządcy w milisekundach (atrapa: ręczny).
pub trait ResidencyClock: Send + Sync {
    /// Milisekundy od dowolnego, stałego punktu.
    fn now_ms(&self) -> u64;
}

/// Ręczny zegar (testy, atrapa): czas płynie tylko przez [`ManualClock::advance_ms`].
#[derive(Debug, Default)]
pub struct ManualClock(std::sync::atomic::AtomicU64);

impl ManualClock {
    /// Zegar od 0 ms.
    pub fn new() -> Self {
        Self::default()
    }

    /// Przesuwa zegar.
    pub fn advance_ms(&self, ms: u64) {
        self.0.fetch_add(ms, std::sync::atomic::Ordering::SeqCst);
    }
}

impl ResidencyClock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Zewnętrzny sygnał trybu (pełny ekran/gra, bateria) — np. `device-profile`.
pub trait ModeSource: Send + Sync {
    /// Czy aktywna jest aplikacja pełnoekranowa (gra).
    fn fullscreen_active(&self) -> bool;
    /// Czy maszyna pracuje na baterii.
    fn on_battery(&self) -> bool;
}

/// Powiadomienia właściciela dzierżawy (wywoływane synchronicznie — nie blokować; zlecić
/// wyładowanie modelu w tle).
pub trait LeaseListener: Send + Sync {
    /// Dzierżawa odebrana: wyładuj model.
    fn revoked(&self, revocation: &Revocation);
    /// Dzierżawa przeniesiona na CPU: przeładuj model na CPU przy najbliższej okazji.
    fn moved(&self, lease: &Lease);
}

/// Zarządca rezydencji. Decyzje synchroniczne (≤ 1 ms, bez ładowania — ładuje klient po przyznaniu).
pub trait Residency: Send + Sync {
    /// Przydziela dzierżawę (z ewentualnymi eksmisjami ustępujących dzierżaw).
    fn acquire(&self, request: LeaseRequest) -> Result<Grant, ResidencyError>;

    /// Zwalnia dzierżawę.
    fn release(&self, id: LeaseId) -> Result<(), ResidencyError>;

    /// Odświeża licznik bezczynności.
    fn touch(&self, id: LeaseId) -> Result<(), ResidencyError>;

    /// Oznacza model jako używany (np. w trakcie tury głosu) albo wolny.
    fn set_in_use(&self, id: LeaseId, in_use: bool) -> Result<(), ResidencyError>;

    /// Aktywna dzierżawa (`None` = zwolniona lub odebrana).
    fn lease(&self, id: LeaseId) -> Option<Lease>;

    /// Co jest gdzie i ile wolne.
    fn snapshot(&self) -> ResidencyState;

    /// Ustawia tryb (gra/bateria/emulacja); zwraca wywołane zmiany.
    fn set_mode(&self, mode: Mode) -> Vec<Change>;

    /// Zmienia budżet maszyny (np. po zmianie sprzętu).
    fn set_budget(&self, budget: Budget) -> Vec<Change>;

    /// Eksmituje dzierżawy bezczynne dłużej niż ich limit.
    fn reap_idle(&self) -> Vec<Revocation>;

    /// Rejestruje słuchacza właściciela (`LeaseRequest::owner`); zastępuje poprzedniego.
    fn listen(&self, owner: &str, listener: Arc<dyn LeaseListener>);

    /// Odświeża tryb z sygnału zewnętrznego (zachowuje emulację); zwraca zmiany.
    fn refresh_mode(&self, source: &dyn ModeSource) -> Vec<Change> {
        let current = self.snapshot().mode;
        let mode = Mode {
            gaming: source.fullscreen_active(),
            battery: source.on_battery(),
            emulated: current.emulated,
        };
        if mode == current {
            Vec::new()
        } else {
            self.set_mode(mode)
        }
    }
}

/// JSON Schema migawki stanu (UI: Ustawienia → Urządzenia).
pub fn state_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ResidencyState)).unwrap_or_default()
}

/// JSON Schema zdarzeń.
pub fn event_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ResidencyEvent)).unwrap_or_default()
}

#[cfg(test)]
mod tests;
