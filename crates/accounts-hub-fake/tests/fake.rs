//! Testy atrapy: kontrakt współdzielony, zgodność stałych skryptu, sterowanie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use accounts_hub_contract::contract_tests::{self, KEY_OK, MODEL_A, MODEL_B, fixture_catalog};
use accounts_hub_contract::{
    AccountSource, AccountState, AccountsHub, Assignments, ConnectionReport, NewAccount,
    ProviderId, SecretName, SecretStore, SecretStoreError, SecretString, TestOutcome,
};
use accounts_hub_fake::{FakeAccountsHub, MapEnv, MemorySecretStore, SCRIPTED_MODELS};

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|catalog| async move { FakeAccountsHub::new(catalog) }).await;
}

#[test]
fn scripted_models_match_contract() {
    assert_eq!(SCRIPTED_MODELS, [MODEL_A, MODEL_B]);
}

#[tokio::test]
async fn controls_and_spy() {
    let hub = FakeAccountsHub::new(fixture_catalog());
    let id = hub
        .add_account(NewAccount {
            provider: ProviderId::new("acme").unwrap(),
            label: String::new(),
            secret: SecretString::from(KEY_OK),
            base_url: None,
            assignments: Assignments::default(),
            cost_limit: None,
            source: AccountSource::Manual,
        })
        .await
        .unwrap();
    assert_eq!(id.as_str(), "acc-1");
    assert_eq!(hub.account(&id).unwrap().label, "Acme AI");
    hub.tester().push(ConnectionReport {
        outcome: TestOutcome::Timeout,
        latency_ms: None,
    });
    assert_eq!(
        hub.test_account(&id).await.unwrap().outcome,
        TestOutcome::Timeout
    );
    assert_eq!(hub.tester().calls(), 1);
    assert!(hub.set_state(&id, AccountState::Active));
    hub.resolve_secret(&id, "providers-api").unwrap();
    assert_eq!(
        hub.secret_reads(),
        vec![(id.clone(), "providers-api".to_owned())]
    );
    assert_eq!(hub.secrets().len(), 1);
    let env = MapEnv::from_pairs(&[("ACME_API_KEY", "")]);
    assert!(
        hub.import_from_env(&env)
            .await
            .unwrap()
            .iter()
            .all(|i| matches!(i.outcome, accounts_hub_contract::EnvImportOutcome::NotSet))
    );
}

#[test]
fn memory_store_fail_injection() {
    let store = MemorySecretStore::new();
    let name = SecretName::new("accounts/acc-1").unwrap();
    store.put(&name, &SecretString::from("v")).unwrap();
    store.fail_next(SecretStoreError::Unavailable("x".into()));
    assert!(store.get(&name).is_err());
    assert_eq!(store.get(&name).unwrap().unwrap().expose_secret(), "v");
    assert_eq!(store.list().unwrap(), vec![name.clone()]);
    assert!(store.delete(&name).unwrap());
    assert!(!store.delete(&name).unwrap());
    assert!(store.is_empty());
}
