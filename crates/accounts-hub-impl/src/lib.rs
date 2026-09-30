//! Hub kont i kluczy (docs/modules/accounts-hub/SPEC.md, PLAN §5.6).
//!
//! `AccountsHubService` implementuje `AccountsHub` i `Module`: katalog z `providers-catalog`
//! walidowany JSON Schema, konta z kluczami wyłącznie w `SecretStore` (produkcyjnie Windows
//! Credential Manager, prefiks `Alfa/`), test połączenia i wykrywanie modeli przez porty
//! z limitem czasu, import kluczy ze zmiennych środowiskowych z katalogu, rotacja/usuwanie
//! bez restartu, zdarzenia `accounts.*` bez wartości kluczy, wykrywanie mostów CLI w PATH.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod catalog_loader;
#[cfg(windows)]
mod credman;
mod hub;
mod ops;
mod repo;
mod system;

use async_trait::async_trait;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError, ModuleManifest};

pub use catalog_loader::{CATALOG_SCHEMA_JSON, CatalogValidator};
#[cfg(windows)]
pub use credman::{CredentialManagerStore, TARGET_PREFIX};
pub use hub::{AccountsHubService, DEFAULT_TEST_TIMEOUT, HubBuilder};
pub use repo::{ACCOUNTS_FILE_VERSION, JsonFileRepository};
pub use system::{ProcessEnv, SystemCliProbe, detect_cli_bridges};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

#[async_trait]
impl Module for AccountsHubService {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut bus = self.bus.write().unwrap_or_else(|p| p.into_inner());
        if bus.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        *bus = Some(ctx.bus);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .map(|_| ())
            .ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        let started = self
            .bus
            .read()
            .map(|b| b.is_some())
            .unwrap_or_else(|p| p.into_inner().is_some());
        if !started {
            return HealthStatus::NotStarted;
        }
        match self.secrets.list() {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => HealthStatus::Degraded(e.to_string()),
        }
    }
}
