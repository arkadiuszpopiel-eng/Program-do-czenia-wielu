//! Moduł `providers-api` w rejestrze: manifest, cykl życia, zdrowie, rejestr dostawców.

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use providers_contract::{HealthState, ModelProvider};

use crate::observe::ObservedProvider;

/// Treść `module.toml` (walidowana testem).
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł adapterów API chmurowych.
pub struct ProvidersApiModule {
    manifest: ModuleManifest,
    providers: Vec<Arc<dyn ModelProvider>>,
    bus: Option<Arc<dyn EventBus>>,
}

impl ProvidersApiModule {
    /// Moduł bez dostawców (program działa bez kluczy — PLAN §5.6).
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            providers: Vec::new(),
            bus: None,
        })
    }

    /// Rejestruje dostawcę (np. zbudowanego z katalogu przez `build_provider`).
    pub fn register(&mut self, provider: Arc<dyn ModelProvider>) {
        self.providers.retain(|p| p.id() != provider.id());
        self.providers.push(provider);
    }

    /// Dostawcy dla Routera; po `start` każde wywołanie publikuje zdarzenia `provider.*`.
    pub fn providers(&self) -> Vec<Arc<dyn ModelProvider>> {
        match &self.bus {
            Some(bus) => self
                .providers
                .iter()
                .map(|p| {
                    Arc::new(ObservedProvider::new(Arc::clone(p), Arc::clone(bus)))
                        as Arc<dyn ModelProvider>
                })
                .collect(),
            None => self.providers.clone(),
        }
    }
}

#[async_trait]
impl Module for ProvidersApiModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        if self.bus.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        self.bus = Some(ctx.bus);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus.take().map(|_| ()).ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        if self.bus.is_none() {
            return HealthStatus::NotStarted;
        }
        let unavailable: Vec<String> = self
            .providers
            .iter()
            .filter(|p| p.health().state == HealthState::Unavailable)
            .map(|p| p.id().to_string())
            .collect();
        if unavailable.is_empty() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded(format!("niedostępni dostawcy: {}", unavailable.join(", ")))
        }
    }
}
