//! Trait `ConfigStore` i typy zmian.

use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;
use serde::{Deserialize, Serialize};

use crate::key::ConfigKey;
use crate::layers::{ConfigLayer, Scope};

/// Wartość konfiguracji (model JSON; pliki źródłowe są TOML).
pub type ConfigValue = serde_json::Value;

/// Kto inicjuje zmianę (SPEC: `Origin::Broker` jako jedyny zmienia polityki Jądra).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", content = "id", rename_all = "kebab-case")]
pub enum Origin {
    /// Użytkownik (UI ustawień).
    User,
    /// Moduł o podanym id.
    Module(String),
    /// Ulepszacz (tylko klucze R0, tylko zmiany zawężające).
    Improver,
    /// Import z pliku `.alfa`.
    Import,
    /// Broker (polityki Jądra).
    Broker,
}

/// Zdarzenie zmiany wartości (dla `watch`; publikowane też jako `config.changed`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigChange {
    /// Zmieniony klucz.
    pub key: ConfigKey,
    /// Zakres.
    pub scope: Scope,
    /// Warstwa, w której nastąpił zapis.
    pub layer: ConfigLayer,
    /// Poprzednia wartość wynikowa (jeśli była).
    pub old: Option<ConfigValue>,
    /// Nowa wartość wynikowa (`None` = usunięto).
    pub new: Option<ConfigValue>,
    /// Inicjator.
    pub origin: Origin,
}

/// Błędy magazynu konfiguracji.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Klucz oznaczony `kernel_policy` — zmienia go tylko Broker.
    #[error("klucz `{0}` jest polityką Jądra; zmienia go wyłącznie Broker")]
    KernelPolicy(ConfigKey),
    /// Wartość nie przeszła walidacji schematu modułu.
    #[error("wartość klucza `{key}` niezgodna ze schematem: {reason}")]
    SchemaViolation {
        /// Klucz.
        key: ConfigKey,
        /// Powód.
        reason: String,
    },
    /// Klucz nieznany żadnemu modułowi.
    #[error("nieznany klucz `{0}`")]
    UnknownKey(ConfigKey),
    /// Błąd trwałego zapisu.
    #[error("błąd zapisu konfiguracji: {0}")]
    Persist(String),
}

/// Reguła polityk Jądra: klucz `kernel.*` zmienia tylko `Origin::Broker` (SPEC, ACC-F0-core-config-02).
/// Wspólna dla `-impl` i `-fake`; wywoływana przed jakąkolwiek inną walidacją zapisu.
pub fn authorize(key: &ConfigKey, origin: &Origin) -> Result<(), ConfigError> {
    if key.is_kernel_policy() && *origin != Origin::Broker {
        return Err(ConfigError::KernelPolicy(key.clone()));
    }
    Ok(())
}

/// Strumień zmian.
pub type ConfigWatch = Pin<Box<dyn Stream<Item = ConfigChange> + Send>>;

/// Magazyn konfiguracji: odczyt wartości wynikowej, zapis do warstwy, obserwacja prefiksu.
#[async_trait]
pub trait ConfigStore: Send + Sync {
    /// Wartość wynikowa (po nałożeniu warstw i zakresu); `None` = brak wartości i domyślnej.
    async fn get(&self, key: &ConfigKey, scope: &Scope)
    -> Result<Option<ConfigValue>, ConfigError>;

    /// Zapis wartości do warstwy w danym zakresie; `None` usuwa nadpisanie.
    async fn set(
        &self,
        key: &ConfigKey,
        value: Option<ConfigValue>,
        scope: &Scope,
        layer: &ConfigLayer,
        origin: Origin,
    ) -> Result<(), ConfigError>;

    /// Strumień zmian kluczy pod prefiksem (`""` = wszystkie).
    fn watch(&self, prefix: &str) -> ConfigWatch;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_broker_changes_kernel_keys() {
        let kernel = ConfigKey::new("kernel.egress.allow").unwrap();
        for origin in [
            Origin::User,
            Origin::Module("x".into()),
            Origin::Improver,
            Origin::Import,
        ] {
            assert_eq!(
                authorize(&kernel, &origin),
                Err(ConfigError::KernelPolicy(kernel.clone()))
            );
        }
        assert_eq!(authorize(&kernel, &Origin::Broker), Ok(()));
        let plain = ConfigKey::new("voice.tts.engine").unwrap();
        assert_eq!(authorize(&plain, &Origin::Improver), Ok(()));
    }

    #[test]
    fn origin_and_change_round_trip() {
        let change = ConfigChange {
            key: ConfigKey::new("voice.stt.engine").unwrap(),
            scope: Scope::Global,
            layer: ConfigLayer::Shared,
            old: None,
            new: Some(serde_json::json!("whisper")),
            origin: Origin::Module("voice-stt".into()),
        };
        let json = serde_json::to_string(&change).unwrap();
        let back: ConfigChange = serde_json::from_str(&json).unwrap();
        assert_eq!(back, change);
    }
}
