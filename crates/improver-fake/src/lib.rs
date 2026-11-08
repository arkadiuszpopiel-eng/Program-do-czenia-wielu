//! Atrapa Ulepszacza (docs/modules/improver/SPEC.md „Fake”): rdzeń z otoczeniem w pamięci.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::Event;
use core_config_contract::ConfigStore;
use evals_contract::EvalGate;
use improver_contract::{
    ApprovalVerifier, ImproverCore, ImproverError, ImproverHost, ImproverPolicy, Proposal,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Otoczenie atrapy: wirtualny zegar, zdarzenia i stan w pamięci.
#[derive(Debug, Default)]
pub struct FakeImproverHost {
    clock: AtomicU64,
    events: Mutex<Vec<Event>>,
    persisted: Mutex<Vec<Proposal>>,
}

impl FakeImproverHost {
    /// Zegar od `start_ms`.
    pub fn new(start_ms: u64) -> Self {
        Self {
            clock: AtomicU64::new(start_ms),
            ..Self::default()
        }
    }

    /// Przesuwa zegar.
    pub fn advance(&self, ms: u64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    /// Wyemitowane zdarzenia.
    pub fn events(&self) -> Vec<Event> {
        lock(&self.events).clone()
    }

    /// Ostatni zapis kolejki propozycji.
    pub fn persisted(&self) -> Vec<Proposal> {
        lock(&self.persisted).clone()
    }
}

impl ImproverHost for FakeImproverHost {
    fn now_ms(&self) -> u64 {
        self.clock.load(Ordering::SeqCst)
    }

    fn emit(&self, events: Vec<Event>) {
        lock(&self.events).extend(events);
    }

    fn persist(&self, proposals: &[Proposal]) {
        *lock(&self.persisted) = proposals.to_vec();
    }
}

/// Ulepszacz-atrapa.
pub type FakeImprover = ImproverCore<FakeImproverHost>;

/// Buduje atrapę z zegarem od `start_ms`.
pub fn fake_improver(
    start_ms: u64,
    config: Arc<dyn ConfigStore>,
    gate: Arc<dyn EvalGate>,
    verifier: Arc<dyn ApprovalVerifier>,
    policy: ImproverPolicy,
) -> Result<(Arc<FakeImproverHost>, FakeImprover), ImproverError> {
    let host = Arc::new(FakeImproverHost::new(start_ms));
    let core = ImproverCore::new(Arc::clone(&host), config, gate, verifier, policy)?;
    Ok((host, core))
}
