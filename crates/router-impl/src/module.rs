//! Moduł `router` w rejestrze: manifest, zdarzenia `router.*` na magistralę (w kolejności).

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use router_contract::{Router, RouterEvent, TaskClass, event_kind};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::routed::RoutedProvider;
use crate::routing::RouterCore;

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł Routera.
pub struct RouterModule {
    manifest: ModuleManifest,
    core: Arc<RouterCore>,
    forwarder: Option<JoinHandle<()>>,
}

fn level(ev: &RouterEvent) -> Level {
    match ev {
        RouterEvent::Decision { .. } | RouterEvent::BreakerClosed { .. } => Level::Debug,
        RouterEvent::Fallback { .. }
        | RouterEvent::BreakerOpened { .. }
        | RouterEvent::NoRoute { .. }
        | RouterEvent::PlanWindowExhausted { .. } => Level::Warn,
    }
}

impl RouterModule {
    /// Moduł nad rdzeniem Routera.
    pub fn new(core: Arc<RouterCore>) -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            core,
            forwarder: None,
        })
    }

    /// Rdzeń (rejestracja dostawców, polityka).
    pub fn core(&self) -> Arc<RouterCore> {
        Arc::clone(&self.core)
    }

    /// Router jako `ModelProvider` dla klasy zadań.
    pub fn provider(&self, class: TaskClass) -> RoutedProvider {
        RoutedProvider::new(Arc::clone(&self.core), class)
    }
}

#[async_trait]
impl Module for RouterModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        if self.forwarder.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        let (tx, mut rx) = mpsc::unbounded_channel::<RouterEvent>();
        self.core.set_event_sink(Some(tx));
        let bus: Arc<dyn EventBus> = ctx.bus;
        self.forwarder = Some(tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                let payload = serde_json::to_value(&ev).unwrap_or_default();
                let event = Event::new(event_kind(ev.name()), level(&ev), payload);
                // Zdarzenia są diagnostyczne: błąd magistrali nie wstrzymuje Routera.
                let _ = bus.publish(event).await;
            }
        }));
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        let task = self.forwarder.take().ok_or(ModuleError::NotStarted)?;
        // Zamknięcie kolejki: przekaźnik wysyła zaległe zdarzenia i kończy się sam.
        self.core.set_event_sink(None);
        let _ = task.await;
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        if self.forwarder.is_none() {
            return HealthStatus::NotStarted;
        }
        let policy = self.core.policy();
        let empty: Vec<String> = router_contract::ALL_CLASSES
            .iter()
            .filter(|c| policy.candidates(**c).is_empty())
            .map(|c| format!("{c:?}"))
            .collect();
        if empty.is_empty() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded(format!("klasy bez tras: {}", empty.join(", ")))
        }
    }
}
