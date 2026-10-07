//! Współdzielony test kontraktowy konfiguracji na `FileConfigStore` (katalog tymczasowy).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use async_trait::async_trait;
use core_config_contract::contract_tests::{self, FIXTURE_PREFIX, Harness, fixture_schema};
use core_config_contract::{
    ConfigError, ConfigKey, ConfigLayer, ConfigStore, ConfigValue, ConfigWatch, MachineId, Origin,
    Scope,
};
use core_config_impl::{ConfigOptions, FileConfigStore};
use tempfile::TempDir;

/// Magazyn razem z katalogiem tymczasowym (katalog żyje tak długo jak magazyn).
struct TempStore {
    inner: FileConfigStore,
    _dir: TempDir,
}

#[async_trait]
impl ConfigStore for TempStore {
    async fn get(
        &self,
        key: &ConfigKey,
        scope: &Scope,
    ) -> Result<Option<ConfigValue>, ConfigError> {
        self.inner.get(key, scope).await
    }
    async fn set(
        &self,
        key: &ConfigKey,
        value: Option<ConfigValue>,
        scope: &Scope,
        layer: &ConfigLayer,
        origin: Origin,
    ) -> Result<(), ConfigError> {
        self.inner.set(key, value, scope, layer, origin).await
    }
    fn watch(&self, prefix: &str) -> ConfigWatch {
        self.inner.watch(prefix)
    }
}

fn harness() -> Harness<TempStore> {
    let dir = tempfile::tempdir().unwrap();
    let machine = MachineId::new("m1");
    let options = ConfigOptions::new(dir.path(), machine.clone());
    let store = FileConfigStore::open(options, Arc::new(chrono::Utc::now)).unwrap();
    store
        .register_schema(&ConfigKey::new(FIXTURE_PREFIX).unwrap(), &fixture_schema())
        .unwrap();
    Harness {
        store: TempStore {
            inner: store,
            _dir: dir,
        },
        machine,
    }
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(harness).await;
}
