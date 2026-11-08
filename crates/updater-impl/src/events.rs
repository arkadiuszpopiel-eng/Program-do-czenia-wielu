//! Zdarzenia na magistralę (kolejka → zadanie tokio publikujące w kolejności) i cykl życia modułu.

use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, EventKind, Level};
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError, ModuleManifest};
use tokio::sync::mpsc;

use crate::FsUpdater;

/// Kolejka zdarzeń: operacje synchroniczne wrzucają zdarzenie, zadanie tokio publikuje je na
/// magistralę. Przed `start` zdarzenia są pomijane (moduł działa bez magistrali, np. w testach).
#[derive(Default)]
pub struct Outbox {
    tx: Mutex<Option<mpsc::UnboundedSender<Event>>>,
}

impl Outbox {
    fn start(&self, bus: Arc<dyn EventBus>) -> Result<(), ModuleError> {
        let mut guard = self.tx.lock().unwrap_or_else(PoisonError::into_inner);
        if guard.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                // Zdarzenia Audytu są informacyjne — błąd magistrali nie cofa przełączenia.
                let _ = bus.publish(event).await;
            }
        });
        *guard = Some(tx);
        Ok(())
    }

    fn stop(&self) -> Result<(), ModuleError> {
        self.tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .map(|_| ())
            .ok_or(ModuleError::NotStarted)
    }

    fn started(&self) -> bool {
        self.tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    /// Publikuje zdarzenie (ładunek bez treści elementów i bez sekretów).
    pub fn emit(&self, kind: &str, payload: serde_json::Value) {
        let guard = self.tx.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(Event::new(
                EventKind::Custom(kind.to_owned()),
                Level::Info,
                payload,
            ));
        }
    }
}

#[async_trait]
impl Module for FsUpdater {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        self.outbox.start(ctx.bus)
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.outbox.stop()
    }

    fn health(&self) -> HealthStatus {
        if self.outbox.started() {
            HealthStatus::Healthy
        } else {
            HealthStatus::NotStarted
        }
    }
}
