//! Windows Credential Manager na prawdziwym systemie. Ignorowany domyślnie (zapisuje do magazynu
//! użytkownika); uruchamiać na self-hosted Windows: `cargo test -p accounts-hub-impl -- --ignored`.

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use accounts_hub_contract::{SecretName, SecretStore, SecretString};
use accounts_hub_impl::CredentialManagerStore;

#[test]
#[ignore = "zapisuje do prawdziwego Credential Manager (self-hosted Windows)"]
fn put_get_list_delete_roundtrip() {
    let store = CredentialManagerStore::new().unwrap();
    let name = SecretName::new(format!("test/roundtrip-{}", std::process::id())).unwrap();
    let value = SecretString::from("sk-test-zażółć-1234");
    store.put(&name, &value).unwrap();
    assert_eq!(
        store.get(&name).unwrap().unwrap().expose_secret(),
        value.expose_secret()
    );
    assert!(store.list().unwrap().contains(&name));
    store
        .put(&name, &SecretString::from("sk-test-rotated"))
        .unwrap();
    assert_eq!(
        store.get(&name).unwrap().unwrap().expose_secret(),
        "sk-test-rotated"
    );
    assert!(store.delete(&name).unwrap());
    assert!(!store.delete(&name).unwrap());
    assert!(store.get(&name).unwrap().is_none());
    assert!(!store.list().unwrap().contains(&name));
}
