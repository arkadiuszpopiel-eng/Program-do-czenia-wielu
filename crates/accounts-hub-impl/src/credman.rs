//! Windows Credential Manager jako `SecretStore` (tylko `cfg(windows)`).
//!
//! Używa `keyring-core` + `windows-native-keyring-store` (bez `unsafe` w kodzie Alfy; ten sam
//! `windows-sys` 0.61 co reszta workspace'u). Każdy wpis to poświadczenie ogólne (generic)
//! o nazwie docelowej `Alfa/<nazwa>` z trwałością `Local` (per maszyna, bez roamingu).
//! Błędy biblioteki są mapowane bez surowych bajtów (wariant `BadEncoding` niesie sekret).

use std::collections::HashMap;
use std::sync::Arc;

use accounts_hub_contract::{SecretName, SecretStore, SecretStoreError, SecretString};
use keyring_core::api::CredentialStoreApi;
use keyring_core::{Entry, Error as KeyringError};
use windows_native_keyring_store::Store;
use zeroize::Zeroize;

/// Prefiks nazw docelowych wszystkich sekretów Alfy.
pub const TARGET_PREFIX: &str = "Alfa/";

/// Magazyn sekretów w Windows Credential Manager.
pub struct CredentialManagerStore {
    store: Arc<Store>,
}

impl CredentialManagerStore {
    /// Otwiera magazyn systemowy.
    pub fn new() -> Result<Self, SecretStoreError> {
        Ok(Self {
            store: Store::new().map_err(|e| map_error(e, "store"))?,
        })
    }

    fn entry(&self, name: &SecretName) -> Result<Entry, SecretStoreError> {
        let target = format!("{TARGET_PREFIX}{}", name.as_str());
        let modifiers = HashMap::from([("target", target.as_str()), ("persistence", "Local")]);
        self.store
            .build("alfa", "", Some(&modifiers))
            .map_err(|e| map_error(e, name.as_str()))
    }
}

fn map_error(error: KeyringError, name: &str) -> SecretStoreError {
    match error {
        KeyringError::BadEncoding(mut bytes) | KeyringError::BadDataFormat(mut bytes, _) => {
            bytes.zeroize();
            SecretStoreError::BadFormat(name.to_owned())
        }
        KeyringError::TooLong(_, max) => SecretStoreError::TooLong {
            max: usize::try_from(max).unwrap_or(usize::MAX),
        },
        KeyringError::NoStorageAccess(e) => SecretStoreError::Unavailable(e.to_string()),
        KeyringError::PlatformFailure(e) => SecretStoreError::Backend(e.to_string()),
        KeyringError::Invalid(param, reason) => {
            SecretStoreError::Backend(format!("niepoprawny parametr `{param}`: {reason}"))
        }
        KeyringError::Ambiguous(_) => {
            SecretStoreError::Backend(format!("niejednoznaczny wpis `{name}`"))
        }
        KeyringError::NoEntry => SecretStoreError::Backend(format!("brak wpisu `{name}`")),
        _ => SecretStoreError::Unavailable("magazyn nie obsługuje operacji".into()),
    }
}

impl SecretStore for CredentialManagerStore {
    fn put(&self, name: &SecretName, value: &SecretString) -> Result<(), SecretStoreError> {
        self.entry(name)?
            .set_password(value.expose_secret())
            .map_err(|e| map_error(e, name.as_str()))
    }

    fn get(&self, name: &SecretName) -> Result<Option<SecretString>, SecretStoreError> {
        match self.entry(name)?.get_password() {
            Ok(value) => Ok(Some(SecretString::new(value))),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(e) => Err(map_error(e, name.as_str())),
        }
    }

    fn delete(&self, name: &SecretName) -> Result<bool, SecretStoreError> {
        match self.entry(name)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(KeyringError::NoEntry) => Ok(false),
            Err(e) => Err(map_error(e, name.as_str())),
        }
    }

    fn list(&self) -> Result<Vec<SecretName>, SecretStoreError> {
        let pattern = format!("^{TARGET_PREFIX}");
        let spec = HashMap::from([("pattern", pattern.as_str())]);
        let entries = self
            .store
            .search(&spec)
            .map_err(|e| map_error(e, TARGET_PREFIX))?;
        let mut names: Vec<SecretName> = entries
            .iter()
            .filter_map(|entry| entry.get_attributes().ok())
            .filter_map(|attrs| {
                attrs
                    .get("target_name")
                    .and_then(|t| t.strip_prefix(TARGET_PREFIX))
                    .and_then(|n| SecretName::new(n).ok())
            })
            .collect();
        names.sort();
        names.dedup();
        Ok(names)
    }
}
