//! Wpis modułu w rejestrze (`core-registry`) dla usług uruchamianych przez kompozycję.
//! Rejestr wylicza graf kontraktów i kolejność startu; usługi startują w tej kolejności,
//! a w rejestrze zostaje ten pośrednik (manifest + zdrowie usługi) — usługa pozostaje
//! współdzielona (`Arc`) z komendami `AppCore`.

use std::sync::Arc;

use async_trait::async_trait;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError, ModuleManifest};

/// Funkcja zdrowia usługi.
pub type HealthFn = Arc<dyn Fn() -> HealthStatus + Send + Sync>;

/// Pośrednik modułu w rejestrze.
pub struct ProxyModule {
    manifest: ModuleManifest,
    health: HealthFn,
}

impl ProxyModule {
    /// Pośrednik z manifestem i funkcją zdrowia.
    pub fn new(manifest: ModuleManifest, health: HealthFn) -> Self {
        Self { manifest, health }
    }
}

#[async_trait]
impl Module for ProxyModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, _ctx: ModuleContext) -> Result<(), ModuleError> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        (self.health)()
    }
}
