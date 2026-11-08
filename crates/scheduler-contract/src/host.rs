//! Otoczenie rdzenia: zegar ścienny, ujście zdarzeń, budzenie sterownika, trwałość stanu
//! i bramka budżetu tła. `-impl` (tokio, magistrala, plik, `cost-meter`) i `-fake` (wirtualny
//! zegar, nagranie, pamięć) różnią się tylko otoczeniem.

use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::Event;
use cost_meter_contract::BudgetDecision;
use scheduler_lite_contract::Host;

use crate::engine::Snapshot;

/// Otoczenie pełnego schedulera.
pub trait SchedHost: Send + Sync + 'static {
    /// Czas ścienny (ms od epoki UTC; monotoniczny w obrębie procesu).
    fn now_ms(&self) -> u64;
    /// Zdarzenia do opublikowania (w kolejności decyzji).
    fn emit(&self, events: Vec<Event>);
    /// Zmienił się stan albo najbliższy termin — sterownik powinien wywołać `pump`.
    fn wake(&self) {}
    /// Trwały zapis stanu (po każdej zmianie; implementacja może łączyć zapisy).
    fn persist(&self, _snapshot: &Snapshot) {}
    /// Decyzja budżetu tła dla zadania klasy `Background` o szacowanym koszcie (mikro-PLN).
    fn check_background_budget(&self, _estimate_micro_pln: u64) -> BudgetDecision {
        BudgetDecision::Allow
    }
}

/// Magazyn stanu (restart = wznowienie).
pub trait SnapshotStore: Send + Sync {
    /// Ostatni zapisany stan.
    fn load(&self) -> Result<Option<Snapshot>, String>;
    /// Zapisuje stan atomowo.
    fn save(&self, snapshot: &Snapshot) -> Result<(), String>;
}

/// Magazyn w pamięci (testy, atrapa).
#[derive(Debug, Default)]
pub struct MemSnapshotStore {
    inner: Mutex<Option<Snapshot>>,
}

impl MemSnapshotStore {
    /// Pusty magazyn.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Option<Snapshot>> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl SnapshotStore for MemSnapshotStore {
    fn load(&self) -> Result<Option<Snapshot>, String> {
        Ok(self.lock().clone())
    }

    fn save(&self, snapshot: &Snapshot) -> Result<(), String> {
        *self.lock() = Some(snapshot.clone());
        Ok(())
    }
}

/// Most otoczenia do warstwy zasobów `scheduler-lite` (ten sam zegar i ujście zdarzeń).
pub struct LiteBridge<H: SchedHost>(pub Arc<H>);

impl<H: SchedHost> Host for LiteBridge<H> {
    fn now_ms(&self) -> u64 {
        self.0.now_ms()
    }

    fn emit(&self, events: Vec<Event>) {
        self.0.emit(events);
    }

    fn deadline_changed(&self) {
        self.0.wake();
    }
}
