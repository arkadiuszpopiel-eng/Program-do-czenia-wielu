//! Implementacja modułu-wzorca „echo” (docs/PLAN.md §3.2, §4.5a pkt 2).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use example_module_contract::{Echo, EchoError, EchoReply, echo_called_kind, validate_input};
use tokio::sync::RwLock;

/// Treść `module.toml` tego modułu (parsowana raz w `EchoModule::new`).
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł echo: liczy wywołania i publikuje zdarzenia na magistralę po `start`.
pub struct EchoModule {
    manifest: ModuleManifest,
    seq: AtomicU64,
    bus: RwLock<Option<Arc<dyn EventBus>>>,
}

impl EchoModule {
    /// Tworzy moduł z manifestem z `module.toml`; błąd tylko przy zepsutym manifeście.
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            seq: AtomicU64::new(0),
            bus: RwLock::new(None),
        })
    }

    /// Liczba udanych wywołań.
    pub fn calls(&self) -> u64 {
        self.seq.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl Echo for EchoModule {
    async fn echo(&self, input: &str) -> Result<EchoReply, EchoError> {
        let bus = self.bus.read().await.clone().ok_or(EchoError::NotStarted)?;
        let chars = validate_input(input)?;
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let reply = EchoReply {
            text: input.to_owned(),
            chars,
            seq,
        };
        let event = Event::new(
            echo_called_kind(),
            Level::Debug,
            serde_json::json!({ "chars": chars, "seq": seq }),
        );
        // Błąd magistrali nie unieważnia echa (zdarzenie jest diagnostyczne).
        let _ = bus.publish(event).await;
        Ok(reply)
    }
}

#[async_trait]
impl Module for EchoModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut bus = self.bus.write().await;
        if bus.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        *bus = Some(ctx.bus);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus
            .write()
            .await
            .take()
            .map(|_| ())
            .ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        match self.bus.try_read() {
            Ok(guard) if guard.is_some() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::NotStarted,
            Err(_) => HealthStatus::Degraded("magistrala zajęta".into()),
        }
    }
}
