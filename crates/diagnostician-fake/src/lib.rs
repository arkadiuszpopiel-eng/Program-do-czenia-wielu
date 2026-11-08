//! Atrapa Diagnosty (docs/modules/diagnostician/SPEC.md „Fake”): rdzeń z otoczeniem w pamięci
//! i świat chaosowy z katalogiem awarii (testy chaosowe na atrapach portów, ACCEPTANCE F8-01).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod catalog;
mod chaos;
mod faults;
mod probe;
mod state;
mod world;

use std::sync::{Arc, Mutex};

use core_bus_contract::Event;
use diagnostician_contract::{
    DiagHost, DiagnosticianCore, JournalEntry, KernelApprovals, RepairContext, RepairEnv,
    RepairPolicy,
};
use watchdog_contract::{Clock, ManualClock};

pub use catalog::{FAULTS, FaultSpec};
pub use chaos::{ChaosReport, ChaosSetup, FaultResult, run_chaos};
pub use faults::inject;
pub use probe::healthy;
pub use state::{ENTRY_CAPACITY, FileInfo, MIN_FREE_MB, WorldState, volume_of};
pub use world::{ChaosBroker, ChaosWorld};

use crate::world::lock;

/// Otoczenie atrapy.
pub struct FakeDiagHost {
    clock: Arc<ManualClock>,
    events: Mutex<Vec<Event>>,
    journal: Mutex<Vec<JournalEntry>>,
}

impl FakeDiagHost {
    /// Otoczenie z zegarem.
    pub fn new(clock: Arc<ManualClock>) -> Self {
        Self {
            clock,
            events: Mutex::new(Vec::new()),
            journal: Mutex::new(Vec::new()),
        }
    }

    /// Wyemitowane zdarzenia.
    pub fn events(&self) -> Vec<Event> {
        lock(&self.events).clone()
    }

    /// Wpisy przekazane do trwałego zapisu.
    pub fn persisted(&self) -> Vec<JournalEntry> {
        lock(&self.journal).clone()
    }

    /// Zegar.
    pub fn clock(&self) -> &Arc<ManualClock> {
        &self.clock
    }
}

impl DiagHost for FakeDiagHost {
    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    fn emit(&self, events: Vec<Event>) {
        lock(&self.events).extend(events);
    }

    fn append(&self, entry: &JournalEntry) {
        lock(&self.journal).push(entry.clone());
    }
}

/// Diagnosta-atrapa.
pub type FakeDiagnostician = DiagnosticianCore<FakeDiagHost>;

/// Buduje atrapę.
pub fn fake_diagnostician(
    clock: Arc<ManualClock>,
    env: Arc<dyn RepairEnv>,
    ctx: Arc<dyn RepairContext>,
    kernel: Arc<dyn KernelApprovals>,
    policy: RepairPolicy,
) -> (Arc<FakeDiagHost>, FakeDiagnostician) {
    let host = Arc::new(FakeDiagHost::new(clock));
    let core = DiagnosticianCore::new(Arc::clone(&host), env, ctx, kernel, policy);
    (host, core)
}

/// Atrapa nad światem chaosowym (środowisko, kontekst i Broker = świat).
pub fn chaos_diagnostician(setup: ChaosSetup) -> (Arc<FakeDiagHost>, FakeDiagnostician) {
    let broker = Arc::new(ChaosBroker(Arc::clone(&setup.world)));
    fake_diagnostician(
        setup.clock,
        setup.world.clone(),
        setup.world,
        broker,
        setup.policy,
    )
}
