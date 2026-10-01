//! Zestaw kontraktowy na atrapie + testy własne (wstrzykiwanie błędów, zapis wywołań i zdarzeń).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};

use model_residency_contract::contract_tests::{self, Harness, request};
use model_residency_contract::{Budget, Mode, Priority, Residency, ResidencyError, ResidencyEvent};
use model_residency_fake::{Call, FakeResidency};

#[derive(Default)]
struct FakeHarness {
    last: Mutex<Option<FakeResidency>>,
}

impl Harness for FakeHarness {
    type R = FakeResidency;

    fn residency(&self, budget: Budget) -> FakeResidency {
        let fake = FakeResidency::new(budget);
        *self.last.lock().unwrap() = Some(fake.clone());
        fake
    }

    fn advance_ms(&self, ms: u64) {
        if let Some(f) = self.last.lock().unwrap().as_ref() {
            f.clock().advance_ms(ms);
        }
    }
}

#[test]
fn contract_suite_on_fake() {
    contract_tests::run_all(&FakeHarness::default());
}

#[test]
fn injected_errors_calls_and_events_are_recorded() {
    let fake = FakeResidency::baseline();
    fake.fail_next(ResidencyError::Wait { blockers: vec![] });
    let req = request("stt", Priority::VoiceRt, 1_500);
    assert!(fake.acquire(req.clone()).is_err());
    let g = fake.acquire(req.clone()).unwrap();
    fake.set_mode(Mode {
        battery: true,
        ..Mode::normal()
    });
    fake.release(g.lease.id).unwrap();
    let calls = fake.calls();
    assert_eq!(calls.len(), 4);
    assert!(matches!(
        &calls[0],
        Call::Acquire(_, Err(ResidencyError::Wait { .. }))
    ));
    assert!(matches!(&calls[1], Call::Acquire(_, Ok(id)) if *id == g.lease.id));
    assert!(matches!(calls[3], Call::Release(id) if id == g.lease.id));
    let names: Vec<&str> = fake.events().iter().map(ResidencyEvent::name).collect();
    assert_eq!(
        names,
        [
            "residency.granted",
            "residency.mode_changed",
            "residency.released"
        ]
    );
    let shared = Arc::new(fake);
    assert_eq!(shared.snapshot().leases.len(), 0);
}
