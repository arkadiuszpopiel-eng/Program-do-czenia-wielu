//! Moduł `model-residency` w rejestrze: manifest, zdarzenia na magistralę, zadanie tła.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, EventFilter, Level};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use device_profile_contract::DeviceProfile;
use futures_util::StreamExt;
use model_residency_contract::{ModeSource, Residency, ResidencyEvent, event_kind};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::config::ResidencyConfig;
use crate::manager::ResidencyManager;

/// Źródło sprzętu dla zmian w trakcie pracy: profil urządzenia i konfiguracja maszyny.
type Hardware = (Arc<dyn DeviceProfile>, ResidencyConfig);

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł zarządcy rezydencji.
pub struct ResidencyModule {
    manifest: ModuleManifest,
    manager: Arc<ResidencyManager>,
    signals: Option<Arc<dyn ModeSource>>,
    hardware: Option<Arc<Hardware>>,
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
            hardware: None,
            tick,
            tasks: Vec::new(),
        })
    }

    /// Zmiany sprzętu w trakcie pracy (C6.6): po zdarzeniu `device.*` (m.in. `device.changed`,
    /// `device.override`) i w każdym kroku zadania tła zarządca czyta profil od nowa i — gdy
    /// sprzęt się zmienił — ustawia budżet i pule kart naraz ([`ResidencyManager::sync_hardware`]).
    #[must_use]
    pub fn with_hardware(
        mut self,
        device: Arc<dyn DeviceProfile>,
        config: ResidencyConfig,
    ) -> Self {
        self.hardware = Some(Arc::new((device, config)));
        self
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
        // Subskrypcja przed przekaźnikiem: zdarzenia `device.*` od chwili startu modułu.
        let device_events = match &self.hardware {
            Some(_) => bus.subscribe(EventFilter::prefix("device.")).await.ok(),
            None => None,
        };
        self.tasks.push(tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                let payload = serde_json::to_value(&ev).unwrap_or_default();
                let event = Event::new(event_kind(ev.name()), level(&ev), payload);
                // Zdarzenia są diagnostyczne: błąd magistrali nie wstrzymuje zarządcy.
                let _ = bus.publish(event).await;
            }
        }));
        if let (Some(mut stream), Some(hw)) = (device_events, self.hardware.clone()) {
            let manager = Arc::clone(&self.manager);
            self.tasks.push(tokio::spawn(async move {
                // Także po pominiętych zdarzeniach (przepełnienie): odczyt sprzętu jest tani.
                while stream.next().await.is_some() {
                    manager.sync_hardware(hw.0.as_ref(), &hw.1);
                }
            }));
        }
        let manager = Arc::clone(&self.manager);
        let signals = self.signals.clone();
        let hardware = self.hardware.clone();
        let tick = self.tick.max(Duration::from_millis(10));
        self.tasks.push(tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                // Zapas dla zmian bez zdarzenia (emulacja, detekcja bez publikacji `device.changed`).
                if let Some(hw) = &hardware {
                    manager.sync_hardware(hw.0.as_ref(), &hw.1);
                }
                Self::tick_once(&manager, signals.as_deref());
            }
        }));
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        if self.tasks.is_empty() {
            return Err(ModuleError::NotStarted);
        }
        // Zadania tła przerywane; kolejka zamykana — przekaźnik (pierwsze zadanie) wysyła zaległe
        // zdarzenia i kończy się.
        self.manager.set_event_sink(None);
        let mut tasks = std::mem::take(&mut self.tasks).into_iter();
        let forwarder = tasks.next();
        for background in tasks {
            background.abort();
        }
        if let Some(forwarder) = forwarder {
            let _ = forwarder.await;
        }
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        if self.tasks.is_empty() {
            return HealthStatus::NotStarted;
        }
        let s = self.manager.snapshot();
        if s.within_budget() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy(format!(
                "przekroczony budżet: {:?} > {:?}, karty: {:?}",
                s.used, s.budget, s.gpus
            ))
        }
    }
}
