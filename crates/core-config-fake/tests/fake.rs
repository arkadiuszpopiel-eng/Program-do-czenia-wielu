//! Testy atrapy konfiguracji: kontrakt współdzielony + fixture'y, symulacja plików, historia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_config_contract::contract_tests::{self, Harness, fixture_defaults};
use core_config_contract::{
    ConfigError, ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope,
};
use core_config_fake::FakeConfigStore;
use futures_util::{FutureExt, StreamExt};
use serde_json::json;

fn harness() -> Harness<FakeConfigStore> {
    let machine = MachineId::new("m1");
    Harness {
        store: FakeConfigStore::new(machine.clone()).with_defaults(fixture_defaults()),
        machine,
    }
}

fn key(s: &str) -> ConfigKey {
    ConfigKey::new(s).unwrap()
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(harness).await;
}

#[tokio::test]
async fn simulated_file_change_notifies_and_guards_kernel() {
    let fake = harness().store;
    let mut watch = fake.watch("test");
    let changes = fake
        .simulate_file_change(
            &ConfigLayer::Shared,
            "[test]\nname = \"zeta\"\nwhen = 2026-01-01\n",
        )
        .unwrap();
    assert_eq!(changes.len(), 2);
    let first = watch.next().now_or_never().flatten().unwrap();
    assert_eq!(first.key, key("test.name"));
    assert_eq!(first.new, Some(json!("zeta")));
    assert_eq!(
        fake.get(&key("test.when"), &Scope::Global).await.unwrap(),
        Some(json!("2026-01-01"))
    );
    let kernel = fake.simulate_file_change(&ConfigLayer::Shared, "[kernel]\nx = 1\n");
    assert!(kernel.is_err());
    assert!(
        fake.simulate_file_change(&ConfigLayer::Default, "")
            .is_err()
    );
    assert!(
        fake.simulate_file_change(&ConfigLayer::Shared, "[x]\napi_key = \"a\"")
            .is_err()
    );
}

#[tokio::test]
async fn history_and_injected_failure() {
    let fake = harness().store;
    let k = key("test.level");
    let g = Scope::Global;
    fake.set(&k, Some(json!(4)), &g, &ConfigLayer::Shared, Origin::User)
        .await
        .unwrap();
    fake.fail_next_set(ConfigError::Persist("dysk pełny".into()));
    let failed = fake
        .set(&k, Some(json!(5)), &g, &ConfigLayer::Shared, Origin::User)
        .await;
    assert_eq!(failed, Err(ConfigError::Persist("dysk pełny".into())));
    fake.set(&k, None, &g, &ConfigLayer::Shared, Origin::Import)
        .await
        .unwrap();
    let history = fake.history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].old, Some(json!(4)));
    assert_eq!(history[1].origin, Origin::Import);
    assert_eq!(fake.get(&k, &g).await.unwrap(), Some(json!(3)));
}
