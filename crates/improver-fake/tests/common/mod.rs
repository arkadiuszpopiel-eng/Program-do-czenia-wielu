//! Budowa uprzęży testów kontraktowych na atrapie (magazyn: `core-config-fake`).

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::sync::Arc;

use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope};
use core_config_fake::FakeConfigStore;
use improver_contract::Improver;
use improver_contract::contract_tests::{Harness, Setup};
use improver_fake::fake_improver;

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

/// Uprząż na atrapie.
pub async fn harness(setup: Setup) -> Harness {
    let store = store(&setup.initial).await;
    let (host, mut core) = fake_improver(
        1_000,
        store.clone(),
        setup.gate.clone(),
        setup.verifier.clone(),
        setup.policy.clone(),
    )
    .unwrap();
    for p in setup.proposers {
        core = core.with_proposer(p);
    }
    let initial = setup.initial.len();
    let history = Arc::clone(&store);
    let events = Arc::clone(&host);
    Harness {
        improver: Arc::new(core) as Arc<dyn Improver>,
        config: store as Arc<dyn ConfigStore>,
        writes: Arc::new(move || {
            history
                .history()
                .into_iter()
                .skip(initial)
                .map(|r| (r.key.as_str().to_owned(), r.origin))
                .collect()
        }),
        events: Arc::new(move || events.events()),
        advance: Arc::new(move |ms| host.advance(ms)),
    }
}
