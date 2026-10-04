//! Regresja Q-5: błąd zapisu metadanych kont nie zostawia stanu w pamięci rozjechanego z dyskiem
//! (zmiana zatwierdzana dopiero po udanym zapisie; dodanie nie zostawia osieroconego klucza).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use accounts_hub_contract::contract_tests::{KEY_OK, fixture_catalog};
use accounts_hub_contract::{
    Account, AccountSource, AccountsError, AccountsHub, AccountsRepository, Assignments,
    NewAccount, ProviderId, SecretStore, SecretString,
};
use accounts_hub_fake::MemorySecretStore;

/// Repozytorium w pamięci; `fail` — każdy zapis kończy się błędem.
#[derive(Default)]
struct FlakyRepo {
    saved: Mutex<Vec<Account>>,
    fail: AtomicBool,
}

impl AccountsRepository for FlakyRepo {
    fn load(&self) -> Result<Vec<Account>, AccountsError> {
        Ok(self.saved.lock().unwrap().clone())
    }

    fn save(&self, accounts: &[Account]) -> Result<(), AccountsError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(AccountsError::Persist("dysk pełny".into()));
        }
        *self.saved.lock().unwrap() = accounts.to_vec();
        Ok(())
    }
}

fn acme(label: &str) -> NewAccount {
    NewAccount {
        provider: ProviderId::new("acme").unwrap(),
        label: label.into(),
        secret: SecretString::from(KEY_OK),
        base_url: None,
        assignments: Assignments::default(),
        cost_limit: None,
        source: AccountSource::Manual,
    }
}

fn hub_with(repo: Arc<FlakyRepo>, secrets: Arc<MemorySecretStore>) -> impl AccountsHub {
    common::builder(fixture_catalog(), secrets)
        .repository(repo)
        .build()
        .unwrap()
}

fn on_disk(repo: &FlakyRepo) -> Vec<Account> {
    repo.saved.lock().unwrap().clone()
}

#[tokio::test]
async fn failed_save_keeps_memory_equal_to_disk() {
    let repo = Arc::new(FlakyRepo::default());
    let secrets = Arc::new(MemorySecretStore::new());
    let hub = hub_with(repo.clone(), secrets.clone());
    let id = hub.add_account(acme("Pierwsze")).await.unwrap();
    assert_eq!(hub.accounts(), on_disk(&repo));

    repo.fail.store(true, Ordering::SeqCst);
    // Zmiana ustawień: błąd zapisu → pamięć bez zmian.
    let err = hub
        .update_settings(&id, "Zmienione", Assignments::default(), None)
        .await
        .unwrap_err();
    assert!(matches!(err, AccountsError::Persist(_)), "{err:?}");
    assert_eq!(hub.accounts(), on_disk(&repo));
    assert_eq!(hub.account(&id).unwrap().label, "Pierwsze");

    // Dodanie: błąd zapisu → brak konta w pamięci i brak osieroconego klucza.
    let keys_before = secrets.list().unwrap();
    assert!(hub.add_account(acme("Drugie")).await.is_err());
    assert_eq!(hub.accounts(), on_disk(&repo));
    assert_eq!(secrets.list().unwrap(), keys_before);

    // Usunięcie: błąd zapisu → konto zostaje w pamięci tak jak na dysku.
    assert!(matches!(
        hub.remove(&id).await,
        Err(AccountsError::Persist(_))
    ));
    assert_eq!(hub.accounts(), on_disk(&repo));
    assert!(hub.account(&id).is_some());

    // Po naprawie dysku usunięcie się udaje i sprząta klucz.
    repo.fail.store(false, Ordering::SeqCst);
    hub.remove(&id).await.unwrap();
    assert!(hub.accounts().is_empty() && on_disk(&repo).is_empty());
    assert!(secrets.list().unwrap().is_empty());
}
