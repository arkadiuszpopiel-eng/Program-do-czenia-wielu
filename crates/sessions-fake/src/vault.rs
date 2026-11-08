//! Sejf kluczy w pamięci (atrapa Credential Manager).

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use lib_sqlstore::DbKey;
use sessions_contract::{KeyVault, VaultError};

/// Sejf w pamięci; klucze zerowane przy usunięciu (drop `DbKey`).
#[derive(Debug, Default)]
pub struct MemoryKeyVault {
    keys: Mutex<BTreeMap<String, DbKey>>,
    fail: Mutex<bool>,
}

impl MemoryKeyVault {
    /// Pusty sejf.
    pub fn new() -> Self {
        Self::default()
    }

    /// Nazwy przechowywanych kluczy (posortowane).
    pub fn names(&self) -> Vec<String> {
        self.keys
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .cloned()
            .collect()
    }

    /// Symuluje niedostępność sejfu (wszystkie operacje → `VaultError::Unavailable`).
    pub fn set_unavailable(&self, unavailable: bool) {
        *self.fail.lock().unwrap_or_else(PoisonError::into_inner) = unavailable;
    }

    fn check(&self) -> Result<(), VaultError> {
        if *self.fail.lock().unwrap_or_else(PoisonError::into_inner) {
            return Err(VaultError::Unavailable("atrapa: sejf wyłączony".into()));
        }
        Ok(())
    }
}

impl KeyVault for MemoryKeyVault {
    fn load(&self, name: &str) -> Result<Option<DbKey>, VaultError> {
        self.check()?;
        Ok(self
            .keys
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(name)
            .cloned())
    }

    fn store(&self, name: &str, key: &DbKey) -> Result<(), VaultError> {
        self.check()?;
        if name.trim().is_empty() {
            return Err(VaultError::InvalidName(name.to_owned()));
        }
        self.keys
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(name.to_owned(), key.clone());
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<bool, VaultError> {
        self.check()?;
        Ok(self
            .keys
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(name)
            .is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sessions_contract::load_or_create_key;

    #[test]
    fn store_load_delete() {
        let vault = MemoryKeyVault::new();
        let key = load_or_create_key(&vault, "a").unwrap();
        assert_eq!(load_or_create_key(&vault, "a").unwrap(), key);
        assert_eq!(vault.names(), vec!["a".to_owned()]);
        assert!(vault.delete("a").unwrap());
        assert!(!vault.delete("a").unwrap());
        assert!(vault.store(" ", &key).is_err());
        vault.set_unavailable(true);
        assert!(vault.load("a").is_err());
    }
}
