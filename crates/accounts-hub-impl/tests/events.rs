//! Zdarzenia na magistrali, test szpiegowski (klucz nigdzie poza magazynem), trwałość metadanych,
//! limit czasu testu, integracja z modułem zgodności, cykl życia modułu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use accounts_hub_contract::contract_tests::{KEY_NET, KEY_OK, KEY_OK_2, fixture_catalog};
use accounts_hub_contract::{
    AccountErrorKind, AccountSource, AccountState, AccountsError, AccountsHub, Assignments,
    EVENT_KEY_ADDED, EVENT_KEY_REMOVED, EVENT_KEY_ROTATED, EVENT_KEY_TESTED, EVENT_STATE_CHANGED,
    NewAccount, ProviderId, SecretStore, SecretString, TestOutcome, event_kind,
};
use accounts_hub_fake::MemorySecretStore;
use accounts_hub_impl::{JsonFileRepository, MODULE_TOML};
use compliance_contract::contract_tests::{fixture_registry, fresh_day};
use compliance_contract::{ProviderApiStatus, ProviderPolicyInput, RouteTags};
use compliance_fake::FakeCompliance;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};

fn acme(key: &str) -> NewAccount {
    NewAccount {
        provider: ProviderId::new("acme").unwrap(),
        label: "Acme".into(),
        secret: SecretString::from(key),
        base_url: None,
        assignments: Assignments::default(),
        cost_limit: None,
        source: AccountSource::Manual,
    }
}

#[tokio::test]
async fn events_published_without_key_values() {
    let mut hub = common::hub(fixture_catalog());
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(hub.manifest().id.clone(), Arc::new(bus.clone()));
    hub.start(ctx).await.unwrap();
    let id = hub.add_account(acme(KEY_OK)).await.unwrap();
    hub.test_account(&id).await.unwrap();
    hub.rotate(&id, SecretString::from(KEY_NET)).await.unwrap();
    hub.test_account(&id).await.unwrap();
    hub.rotate(&id, SecretString::from(KEY_OK_2)).await.unwrap();
    hub.remove(&id).await.unwrap();
    let count = |name| bus.recorded_of_kind(&event_kind(name)).len();
    assert_eq!(count(EVENT_KEY_ADDED), 1);
    assert_eq!(count(EVENT_KEY_TESTED), 2);
    assert_eq!(count(EVENT_KEY_ROTATED), 2);
    assert_eq!(count(EVENT_KEY_REMOVED), 1);
    assert!(count(EVENT_STATE_CHANGED) >= 3);
    let tested = bus.recorded_of_kind(&event_kind(EVENT_KEY_TESTED));
    assert_eq!(tested[0].payload["outcome"], "ok");
    assert_eq!(tested[0].payload["models"], 2);
    assert_eq!(tested[1].payload["outcome"], "network");
    let dump: String = bus
        .recorded()
        .iter()
        .map(|e| serde_json::to_string(e.as_ref()).unwrap())
        .collect();
    for key in [KEY_OK, KEY_NET, KEY_OK_2] {
        assert!(!dump.contains(key), "klucz w zdarzeniach: {dump}");
    }
}

#[tokio::test]
async fn metadata_file_has_no_secrets_and_survives_restart() {
    let dir = common::temp_dir("repo");
    let path = dir.join("accounts.json");
    let secrets = Arc::new(MemorySecretStore::new());
    let repo = Arc::new(JsonFileRepository::new(&path));
    let hub = common::builder(fixture_catalog(), secrets.clone())
        .repository(repo.clone())
        .build()
        .unwrap();
    let a = hub.add_account(acme(KEY_OK)).await.unwrap();
    let b = hub.add_account(acme(KEY_OK_2)).await.unwrap();
    hub.test_account(&a).await.unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains(KEY_OK) && !text.contains(KEY_OK_2), "{text}");
    assert!(text.contains("accounts/acc-1"));

    // Klucz konta `b` znika z magazynu (np. metadane przeniesione na inną maszynę).
    secrets.delete(&hub.account(&b).unwrap().secret).unwrap();
    let restarted = common::builder(fixture_catalog(), secrets.clone())
        .repository(repo)
        .build()
        .unwrap();
    assert_eq!(restarted.accounts().len(), 2);
    assert_eq!(restarted.account(&a).unwrap().state, AccountState::Active);
    assert_eq!(
        restarted.account(&b).unwrap().state,
        AccountState::Unconfigured
    );
    assert!(matches!(
        restarted.test_account(&b).await,
        Err(AccountsError::SecretMissing(_))
    ));
    assert_eq!(restarted.account(&a).unwrap().models.len(), 2);
    std::fs::write(&path, "{\"version\": 9, \"accounts\": []}").unwrap();
    assert!(
        common::builder(fixture_catalog(), secrets)
            .repository(Arc::new(JsonFileRepository::new(&path)))
            .build()
            .is_err()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn slow_tester_times_out() {
    let hub = common::builder(fixture_catalog(), Arc::new(MemorySecretStore::new()))
        .test_timeout(Duration::from_millis(50))
        .build()
        .unwrap();
    let id = hub.add_account(acme("sk-slow-0001")).await.unwrap();
    let summary = hub.test_account(&id).await.unwrap();
    assert_eq!(summary.outcome, TestOutcome::Timeout);
    assert_eq!(
        hub.account(&id).unwrap().state,
        AccountState::Error {
            kind: AccountErrorKind::Timeout
        }
    );
}

#[tokio::test]
async fn compliance_can_forbid_provider_api() {
    let mut catalog = fixture_catalog();
    let policy: Vec<ProviderPolicyInput> = catalog.iter().map(|e| e.policy_input()).collect();
    let compliance = FakeCompliance::new(
        fixture_registry(),
        policy
            .into_iter()
            .map(|mut p| {
                if p.provider == "acme" {
                    p.api_status = ProviderApiStatus::Forbidden;
                    p.tags = RouteTags::default();
                }
                p
            })
            .collect(),
        fresh_day(),
    );
    catalog.retain(|e| e.id.as_str() != "banned");
    let hub = common::builder(catalog, Arc::new(MemorySecretStore::new()))
        .compliance(Arc::new(compliance))
        .build()
        .unwrap();
    assert!(matches!(
        hub.add_account(acme(KEY_OK)).await,
        Err(AccountsError::ProviderForbidden(_))
    ));
}

#[tokio::test]
async fn secret_store_errors_propagate_and_lifecycle() {
    let secrets = Arc::new(MemorySecretStore::new());
    let mut hub = common::builder(fixture_catalog(), secrets.clone())
        .build()
        .unwrap();
    secrets.fail_next(accounts_hub_contract::SecretStoreError::Unavailable(
        "brak".into(),
    ));
    assert!(matches!(
        hub.add_account(acme(KEY_OK)).await,
        Err(AccountsError::Secret(_))
    ));
    assert!(hub.accounts().is_empty());
    assert_eq!(hub.health(), HealthStatus::NotStarted);
    assert_eq!(hub.stop().await, Err(ModuleError::NotStarted));
    let ctx = ModuleContext::new(hub.manifest().id.clone(), Arc::new(FakeBus::default()));
    hub.start(ctx.clone()).await.unwrap();
    assert_eq!(hub.start(ctx).await, Err(ModuleError::AlreadyStarted));
    assert_eq!(hub.health(), HealthStatus::Healthy);
    secrets.fail_next(accounts_hub_contract::SecretStoreError::Unavailable(
        "brak".into(),
    ));
    assert!(matches!(hub.health(), HealthStatus::Degraded(_)));
    hub.stop().await.unwrap();
    assert_eq!(hub.manifest().id.as_str(), "accounts-hub");
    assert!(MODULE_TOML.contains("secrets.write"));
}
