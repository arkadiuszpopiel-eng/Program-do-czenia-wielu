//! Sekrety: sejf kluczy baz sesji (`KeyVault`) na magazynie `accounts-hub` (`SecretStore` —
//! Windows Credential Manager) oraz źródło klucza API dla adapterów dostawców.
//! Poza Windows (dev/testy) magazyn jest wyłącznie w pamięci procesu — nic nie trafia na dysk.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use accounts_hub_contract::{
    AccountId, AccountsHub, SecretName, SecretStore, SecretStoreError, SecretString,
};
use lib_sqlstore::DbKey;
use providers_contract::{ApiKey, SecretSource};
use sessions_contract::{KeyVault, VaultError};

/// Moduł, w imieniu którego adaptery czytają klucze (prefiks `providers-` wymagany przez hub).
pub const SECRET_CALLER: &str = "providers-api";

/// `KeyVault` sesji na `SecretStore` (klucz 32 B zapisany jako hex pod nazwą `vault/<nazwa>`).
pub struct StoreKeyVault {
    store: Arc<dyn SecretStore>,
}

impl StoreKeyVault {
    /// Sejf na magazynie sekretów.
    pub fn new(store: Arc<dyn SecretStore>) -> Self {
        Self { store }
    }

    fn name(name: &str) -> Result<SecretName, VaultError> {
        SecretName::new(format!("vault/{name}"))
            .map_err(|_| VaultError::InvalidName(name.to_owned()))
    }
}

fn unavailable(e: SecretStoreError) -> VaultError {
    VaultError::Unavailable(e.to_string())
}

impl KeyVault for StoreKeyVault {
    fn load(&self, name: &str) -> Result<Option<DbKey>, VaultError> {
        let Some(secret) = self.store.get(&Self::name(name)?).map_err(unavailable)? else {
            return Ok(None);
        };
        DbKey::from_hex(secret.expose_secret())
            .map(Some)
            .map_err(|e| VaultError::Corrupted(e.to_string()))
    }

    fn store(&self, name: &str, key: &DbKey) -> Result<(), VaultError> {
        let hex = key.to_hex();
        let value = SecretString::new(hex.as_str().to_owned());
        self.store
            .put(&Self::name(name)?, &value)
            .map_err(unavailable)
    }

    fn delete(&self, name: &str) -> Result<bool, VaultError> {
        self.store.delete(&Self::name(name)?).map_err(unavailable)
    }
}

/// Magazyn sekretów w pamięci procesu (testy, tryb deweloperski poza Windows).
#[derive(Default)]
pub struct MemorySecretStore {
    entries: Mutex<BTreeMap<SecretName, SecretString>>,
}

impl MemorySecretStore {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<SecretName, SecretString>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl SecretStore for MemorySecretStore {
    fn put(&self, name: &SecretName, value: &SecretString) -> Result<(), SecretStoreError> {
        self.lock().insert(name.clone(), value.clone());
        Ok(())
    }

    fn get(&self, name: &SecretName) -> Result<Option<SecretString>, SecretStoreError> {
        Ok(self.lock().get(name).cloned())
    }

    fn delete(&self, name: &SecretName) -> Result<bool, SecretStoreError> {
        Ok(self.lock().remove(name).is_some())
    }

    fn list(&self) -> Result<Vec<SecretName>, SecretStoreError> {
        Ok(self.lock().keys().cloned().collect())
    }
}

/// Magazyn systemowy: Windows Credential Manager; poza Windows — pamięć procesu.
pub fn system_secret_store() -> Result<Arc<dyn SecretStore>, SecretStoreError> {
    #[cfg(windows)]
    {
        Ok(Arc::new(accounts_hub_impl::CredentialManagerStore::new()?))
    }
    #[cfg(not(windows))]
    {
        tracing::warn!("brak Windows Credential Manager — sekrety tylko w pamięci procesu");
        Ok(Arc::new(MemorySecretStore::default()))
    }
}

/// Klucz API konta czytany z huba w chwili wywołania (rotacja bez restartu, PLAN §5.6).
pub struct HubKeySource {
    hub: Arc<dyn AccountsHub>,
    account: AccountId,
}

impl HubKeySource {
    /// Źródło klucza konta.
    pub fn new(hub: Arc<dyn AccountsHub>, account: AccountId) -> Self {
        Self { hub, account }
    }
}

impl SecretSource for HubKeySource {
    fn api_key(&self) -> Option<ApiKey> {
        let secret = self.hub.resolve_secret(&self.account, SECRET_CALLER).ok()?;
        if secret.is_empty() {
            return None;
        }
        Some(ApiKey::new(secret.expose_secret()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_roundtrips_keys_through_secret_store() {
        let store: Arc<dyn SecretStore> = Arc::new(MemorySecretStore::default());
        let vault = StoreKeyVault::new(store.clone());
        assert!(vault.load("alfa/sessions/index").unwrap().is_none());
        let key = DbKey::generate().unwrap();
        vault.store("alfa/sessions/index", &key).unwrap();
        let back = vault.load("alfa/sessions/index").unwrap().unwrap();
        assert_eq!(back.as_bytes(), key.as_bytes());
        let names: Vec<String> = store
            .list()
            .unwrap()
            .iter()
            .map(|n| n.as_str().to_owned())
            .collect();
        assert_eq!(names, vec!["vault/alfa/sessions/index".to_owned()]);
        assert!(vault.delete("alfa/sessions/index").unwrap());
        assert!(vault.load("bad..name").is_err());
    }
}
