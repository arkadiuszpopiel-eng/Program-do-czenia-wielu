//! Uprząż testów kontraktowych na implementacji (magazyn: `core-config-fake`).

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::sync::Arc;

use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope};
use core_config_fake::FakeConfigStore;
use evals_contract::ManualClock;
use improver_contract::Improver;
use improver_contract::contract_tests::{Harness, Setup};
use improver_impl::ImproverService;

/// Magazyn z wartościami początkowymi (`kernel.*` zapisuje „Broker”, reszta „użytkownik”).
pub async fn store(initial: &[(String, serde_json::Value)]) -> Arc<FakeConfigStore> {
    let store = Arc::new(FakeConfigStore::new(MachineId::new("test")));
    for (key, value) in initial {
        let key = ConfigKey::new(key.as_str()).unwrap();
        let origin = if key.is_kernel_policy() {
            Origin::Broker
        } else {
            Origin::User
        };
        store
            .set(
                &key,
                Some(value.clone()),
                &Scope::Global,
                &ConfigLayer::Shared,
                origin,
            )
            .await
            .unwrap();
    }
    store
}

/// Uprząż na implementacji (bez magistrali — zdarzenia zbiera wektor z `cycle`/testów usługi).
pub async fn harness(setup: Setup) -> Harness {
    let store = store(&setup.initial).await;
    let clock = Arc::new(ManualClock::new(1_000));
    let service = ImproverService::with_proposers(
        store.clone(),
        setup.gate.clone(),
        setup.verifier.clone(),
        setup.policy.clone(),
        clock.clone(),
        None,
        setup.proposers,
    )
    .unwrap();
    let service = Arc::new(service);
    let initial = setup.initial.len();
    let history = Arc::clone(&store);
    let recent = Arc::clone(&service);
    Harness {
        improver: service as Arc<dyn Improver>,
        config: store as Arc<dyn ConfigStore>,
        writes: Arc::new(move || {
            history
                .history()
                .into_iter()
                .skip(initial)
                .map(|r| (r.key.as_str().to_owned(), r.origin))
                .collect()
        }),
        events: Arc::new(move || recent.recent_events()),
        advance: Arc::new(move |ms| clock.advance(ms)),
    }
}
