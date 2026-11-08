//! Wymiana klucza konta, którego stan wynikał ze starego klucza (klucz zniknął z magazynu — „nieskonfigurowane”,
//! albo błąd testu), gdy nowego klucza nie da się sprawdzić z góry (własny adres bez listy modeli). Przegląd
//! 2026-10-22: konto zostawało w starym stanie z zapisanym nowym kluczem i Router go nie używał.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use accounts_hub_contract::contract_tests::{KEY_INVALID, KEY_OK, KEY_OK_2, fixture_catalog};
use accounts_hub_contract::{
    AccountSource, AccountState, AccountsError, AccountsHub, Assignments, ConnectionReport,
    NewAccount, ProviderId, SecretStore, SecretString, TestOutcome,
};
use accounts_hub_fake::{MemorySecretStore, ScriptedConnectionTester, ScriptedModelLister};
use accounts_hub_impl::AccountsHubService;

fn acme(key: &str) -> NewAccount {
    NewAccount {
        provider: ProviderId::new("acme").unwrap(),
        label: "Acme".into(),
        secret: SecretString::from(key),
        base_url: None,
        assignments: Assignments::default(),
        cost_limit: None,
        source: AccountSource::Manual,
        enterprise: Default::default(),
    }
}

fn unsupported() -> ConnectionReport {
    ConnectionReport {
        outcome: TestOutcome::Unsupported {
            message: "brak listy modeli pod tym adresem".into(),
        },
        latency_ms: None,
        detail: None,
    }
}

fn hub() -> (
    AccountsHubService,
    Arc<MemorySecretStore>,
    Arc<ScriptedConnectionTester>,
) {
    let secrets = Arc::new(MemorySecretStore::new());
    let tester = Arc::new(ScriptedConnectionTester::by_key_prefix());
    let hub = AccountsHubService::builder(
        fixture_catalog(),
        secrets.clone(),
        tester.clone(),
        Arc::new(ScriptedModelLister::by_key_prefix()),
    )
    .build()
    .unwrap();
    (hub, secrets, tester)
}

#[tokio::test]
async fn uncheckable_new_key_ends_the_missing_key_state() {
    let (hub, secrets, tester) = hub();
    let id = hub.add_account(acme(KEY_OK)).await.unwrap();
    hub.test_account(&id).await.unwrap();
    let name = hub.account(&id).unwrap().secret;

    // Klucz znika z magazynu: test odmawia, konto czeka na klucz.
    secrets.delete(&name).unwrap();
    assert!(matches!(
        hub.test_account(&id).await,
        Err(AccountsError::SecretMissing(_))
    ));
    assert_eq!(hub.account(&id).unwrap().state, AccountState::Unconfigured);

    // Nowego klucza nie da się sprawdzić z góry: zapisany, a konto znów jest w użyciu.
    tester.push(unsupported());
    let summary = hub
        .try_rotate(&id, SecretString::from(KEY_OK_2))
        .await
        .unwrap();
    assert!(matches!(summary.outcome, TestOutcome::Unsupported { .. }));
    assert_eq!(
        secrets.get(&name).unwrap().unwrap().expose_secret(),
        KEY_OK_2
    );
    assert_eq!(hub.account(&id).unwrap().state, AccountState::Active);

    // Konto wyłączone zostaje wyłączone, a działające — działające (wynik „nie da się sprawdzić” nic nie zmienia).
    hub.set_disabled(&id, true).await.unwrap();
    tester.push(unsupported());
    hub.try_rotate(&id, SecretString::from(KEY_OK))
        .await
        .unwrap();
    assert_eq!(hub.account(&id).unwrap().state, AccountState::Disabled);
    hub.set_disabled(&id, false).await.unwrap();
    tester.push(unsupported());
    hub.try_rotate(&id, SecretString::from(KEY_OK_2))
        .await
        .unwrap();
    assert_eq!(hub.account(&id).unwrap().state, AccountState::Active);
}

#[tokio::test]
async fn uncheckable_new_key_ends_the_error_of_the_old_key() {
    let (hub, _secrets, tester) = hub();
    let id = hub.add_account(acme(KEY_INVALID)).await.unwrap();
    hub.test_account(&id).await.unwrap();
    assert!(matches!(
        hub.account(&id).unwrap().state,
        AccountState::Error { .. }
    ));
    tester.push(unsupported());
    hub.try_rotate(&id, SecretString::from(KEY_OK_2))
        .await
        .unwrap();
    assert_eq!(hub.account(&id).unwrap().state, AccountState::Active);
    // Kandydat odrzucony przez dostawcę nie zastępuje klucza i nie zmienia stanu.
    hub.try_rotate(&id, SecretString::from(KEY_INVALID))
        .await
        .unwrap();
    assert_eq!(hub.account(&id).unwrap().state, AccountState::Active);
}
