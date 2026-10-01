//! Moduł `providers-local` w rejestrze: manifest, zdarzenia `local.*` na magistralę,
//! zadanie tła (zwalnianie bezczynnego sidecara), pobieranie modeli ze zdarzeniami postępu.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use providers_contract::CancellationToken;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::download::{Downloaded, Downloader};
use crate::error::{DownloadProgress, LocalError, LocalEvent};
use crate::provider::LocalProvider;

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Co ile bajtów publikować postęp pobierania.
const PROGRESS_STEP: u64 = 8 * 1024 * 1024;

/// Moduł dostawcy lokalnego.
pub struct LocalModule {
    manifest: ModuleManifest,
    provider: Arc<LocalProvider>,
    tick: Duration,
    forwarder: Option<JoinHandle<()>>,
    ticker: Option<JoinHandle<()>>,
}

fn level(ev: &LocalEvent) -> Level {
    match ev {
        LocalEvent::DownloadFailed { .. }
        | LocalEvent::SidecarCrashed { .. }
        | LocalEvent::BackendFallback { .. } => Level::Warn,
        LocalEvent::DownloadProgress { .. } => Level::Debug,
        _ => Level::Info,
    }
}

impl LocalModule {
    /// Moduł nad dostawcą; `tick` = okres sprawdzania bezczynności.
    pub fn new(provider: Arc<LocalProvider>, tick: Duration) -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            provider,
            tick,
            forwarder: None,
            ticker: None,
        })
    }

    /// Dostawca (rejestracja w Routerze jako trasa `Local`).
    pub fn provider(&self) -> Arc<LocalProvider> {
        Arc::clone(&self.provider)
    }

    /// Pobiera model z manifestu (wznawianie, SHA-256) i publikuje `local.model.download.*`.
    pub async fn download(
        &self,
        model: &str,
        cancel: &CancellationToken,
    ) -> Result<Downloaded, LocalError> {
        let entry = self
            .provider
            .entry(model)
            .cloned()
            .ok_or_else(|| LocalError::UnknownModel(model.into()))?;
        let sidecar = Arc::clone(self.provider.sidecar());
        let downloader = Downloader::new(sidecar.config().models_dir.clone())?;
        let last = Mutex::new(0u64);
        let id = entry.id.clone();
        let report = |p: DownloadProgress| {
            let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
            let done = p.total.is_some_and(|t| p.bytes >= t);
            if p.bytes == 0 || done || p.bytes >= last.saturating_add(PROGRESS_STEP) {
                *last = p.bytes;
                sidecar.emit(LocalEvent::DownloadProgress {
                    model: id.clone(),
                    progress: p,
                });
            }
        };
        match downloader.download(&entry, cancel, &report).await {
            Ok(d) => {
                sidecar.emit(LocalEvent::DownloadFinished {
                    model: entry.id.clone(),
                    sha256: d.sha256.clone(),
                    verified: d.verified,
                });
                Ok(d)
            }
            Err(e) => {
                sidecar.emit(LocalEvent::DownloadFailed {
                    model: entry.id.clone(),
                    error: e.to_string(),
                });
                Err(e)
            }
        }
    }
}

#[async_trait]
impl Module for LocalModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        if self.forwarder.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        let (tx, mut rx) = mpsc::unbounded_channel::<LocalEvent>();
        self.provider.sidecar().set_event_sink(Some(tx));
        let bus: Arc<dyn EventBus> = ctx.bus;
        self.forwarder = Some(tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                let payload = serde_json::to_value(&ev).unwrap_or_default();
                let kind = core_bus_contract::EventKind::Custom(ev.name().to_owned());
                // Zdarzenia są diagnostyczne: błąd magistrali nie wstrzymuje dostawcy.
                let _ = bus.publish(Event::new(kind, level(&ev), payload)).await;
            }
        }));
        let sidecar = Arc::clone(self.provider.sidecar());
        let tick = self.tick.max(Duration::from_millis(10));
        self.ticker = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick);
            loop {
                interval.tick().await;
                sidecar.reap_idle_at(tokio::time::Instant::now()).await;
            }
        }));
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        let forwarder = self.forwarder.take().ok_or(ModuleError::NotStarted)?;
        if let Some(t) = self.ticker.take() {
            t.abort();
        }
        let sidecar = self.provider.sidecar();
        sidecar.stop("zatrzymanie modułu").await;
        // Zamknięcie kolejki: przekaźnik wysyła zaległe zdarzenia i kończy się sam.
        sidecar.set_event_sink(None);
        let _ = forwarder.await;
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        if self.forwarder.is_none() {
            return HealthStatus::NotStarted;
        }
        if self.provider.installed().is_empty() {
            HealthStatus::Degraded("brak pobranego modelu lokalnego".into())
        } else {
            HealthStatus::Healthy
        }
    }
}
