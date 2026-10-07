//! Moduł `model-residency` w rejestrze: manifest, zdarzenia na magistralę, zadanie tła.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use model_residency_contract::{ModeSource, Residency, ResidencyEvent, event_kind, fits};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::manager::ResidencyManager;

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł zarządcy rezydencji.
pub struct ResidencyModule {
    manifest: ModuleManifest,
    manager: Arc<ResidencyManager>,
    signals: Option<Arc<dyn ModeSource>>,
    tick: Duration,
    tasks: Vec<JoinHandle<()>>,
}

fn level(ev: &ResidencyEvent) -> Level {
    match ev {
        ResidencyEvent::Evicted { .. }
        | ResidencyEvent::OomAvoided { .. }
        | ResidencyEvent::BudgetExceeded { .. } => Level::Warn,
        ResidencyEvent::ModeChanged { .. } | ResidencyEvent::Moved { .. } => Level::Info,
        ResidencyEvent::Granted { .. } | ResidencyEvent::Released { .. } => Level::Debug,
    }
}

impl ResidencyModule {
    /// Moduł nad zarządcą; `signals` = źródło trybu (np. [`crate::DeviceSignals`]).
    pub fn new(
        manager: Arc<ResidencyManager>,
        signals: Option<Arc<dyn ModeSource>>,
        tick: Duration,
    ) -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            manager,
            signals,
            tick,
            tasks: Vec::new(),
        })
    }

    /// Zarządca (dla klientów: `voice-*`, `providers-local`, `search`).
    pub fn manager(&self) -> Arc<ResidencyManager> {
        Arc::clone(&self.manager)
    }

    /// Jeden krok zadania tła: bezczynność + tryb z sygnałów.
    pub fn tick_once(manager: &ResidencyManager, signals: Option<&dyn ModeSource>) {
        manager.reap_idle();
        if let Some(s) = signals {
            manager.refresh_mode(s);
        }
    }
}

#[async_trait]
impl Module for ResidencyModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        if !self.tasks.is_empty() {
            return Err(ModuleError::AlreadyStarted);
        }
        let (tx, mut rx) = mpsc::unbounded_channel::<ResidencyEvent>();
        self.manager.set_event_sink(Some(tx));
        let bus: Arc<dyn EventBus> = ctx.bus;
        self.tasks.push(tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                let payload = serde_json::to_value(&ev).unwrap_or_default();
                let event = Event::new(event_kind(ev.name()), level(&ev), payload);
                // Zdarzenia są diagnostyczne: błąd magistrali nie wstrzymuje zarządcy.
                let _ = bus.publish(event).await;
            }
        }));
        let manager = Arc::clone(&self.manager);
        let signals = self.signals.clone();
        let tick = self.tick.max(Duration::from_millis(10));
        self.tasks.push(tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                Self::tick_once(&manager, signals.as_deref());
            }
        }));
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        if self.tasks.is_empty() {
            return Err(ModuleError::NotStarted);
        }
        // Zadanie tła przerywane; kolejka zamykana — przekaźnik wysyła zaległe zdarzenia i kończy się.
        self.manager.set_event_sink(None);
        let mut tasks = std::mem::take(&mut self.tasks);
        if let Some(ticker) = tasks.pop() {
            ticker.abort();
        }
        for forwarder in tasks {
            let _ = forwarder.await;
        }
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        if self.tasks.is_empty() {
            return HealthStatus::NotStarted;
        }
        let s = self.manager.snapshot();
        if fits(s.used, s.budget) {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy(format!(
                "przekroczony budżet: {:?} > {:?}",
                s.used, s.budget
            ))
        }
    }
}
