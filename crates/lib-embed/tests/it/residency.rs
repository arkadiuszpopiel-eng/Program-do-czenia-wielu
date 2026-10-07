//! Zarządca rezydencji do testów. `lib-*` może zależeć wyłącznie od `lib-*` i `*-contract` (także
//! w dev — `scripts/check-deps.sh`), więc zamiast `model-residency-fake`: maszyna stanów `LeaseTable`
//! z kontraktu (te same reguły co `-impl` i `-fake`), ręczny zegar, słuchacze właścicieli.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use model_residency_contract::{
    Budget, Change, Grant, Lease, LeaseId, LeaseListener, LeaseRequest, LeaseTable, ManualClock,
    Mode, Residency, ResidencyClock, ResidencyError, ResidencyState, Revocation,
};

/// Zarządca testowy.
pub struct TableResidency {
    table: Mutex<LeaseTable>,
    listeners: Mutex<BTreeMap<String, Arc<dyn LeaseListener>>>,
    pub clock: ManualClock,
}

impl TableResidency {
    pub fn new(ram_mb: u32) -> Self {
        Self {
            table: Mutex::new(LeaseTable::new(Budget {
                vram_mb: 0,
                ram_mb,
                desktop_reserve_mb: 0,
                stt_tts_exclusive: false,
            })),
            listeners: Mutex::new(BTreeMap::new()),
            clock: ManualClock::new(),
        }
    }

    fn notify(&self, changes: &[Change]) {
        let listeners = self.listeners.lock().unwrap().clone();
        for change in changes {
            if let Change::Revoked(r) = change
                && let Some(l) = listeners.get(&r.lease.request.owner)
            {
                l.revoked(r);
            }
        }
    }
}

impl Residency for TableResidency {
    fn acquire(&self, request: LeaseRequest) -> Result<Grant, ResidencyError> {
        let grant = self
            .table
            .lock()
            .unwrap()
            .acquire(&request, self.clock.now_ms())?;
        let changes: Vec<Change> = grant.evicted.iter().cloned().map(Change::Revoked).collect();
        self.notify(&changes);
        Ok(grant)
    }

    fn release(&self, id: LeaseId) -> Result<(), ResidencyError> {
        self.table.lock().unwrap().release(id).map(|_| ())
    }

    fn touch(&self, id: LeaseId) -> Result<(), ResidencyError> {
        self.table.lock().unwrap().touch(id, self.clock.now_ms())
    }

    fn set_in_use(&self, id: LeaseId, in_use: bool) -> Result<(), ResidencyError> {
        self.table
            .lock()
            .unwrap()
            .set_in_use(id, in_use, self.clock.now_ms())
    }

    fn lease(&self, id: LeaseId) -> Option<Lease> {
        self.table.lock().unwrap().lease(id).cloned()
    }

    fn snapshot(&self) -> ResidencyState {
        self.table.lock().unwrap().snapshot()
    }

    fn set_mode(&self, mode: Mode) -> Vec<Change> {
        let changes = self.table.lock().unwrap().set_mode(mode);
        self.notify(&changes);
        changes
    }

    fn set_budget(&self, budget: Budget) -> Vec<Change> {
        let changes = self.table.lock().unwrap().set_budget(budget);
        self.notify(&changes);
        changes
    }

    fn reap_idle(&self) -> Vec<Revocation> {
        let reaped = self.table.lock().unwrap().reap_idle(self.clock.now_ms());
        let changes: Vec<Change> = reaped.iter().cloned().map(Change::Revoked).collect();
        self.notify(&changes);
        reaped
    }

    fn listen(&self, owner: &str, listener: Arc<dyn LeaseListener>) {
        self.listeners
            .lock()
            .unwrap()
            .insert(owner.to_owned(), listener);
    }
}
