//! Zdarzenia na magistralę (kolejka → zadanie tokio) i cykl życia modułu.

use std::sync::{Arc, Mutex, PoisonError};

use artifacts_contract::SessionId;
use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, EventKind, Level};
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError, ModuleManifest};
use tokio::sync::mpsc;

use crate::SqliteArtifacts;

/// Kolejka zdarzeń publikowanych w kolejności; przed `start` zdarzenia są pomijane.
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

    /// Publikuje zdarzenie (ładunek wyłącznie z licznikami — bez treści zapytań i dokumentów).
    pub fn emit(
        &self,
        kind: &str,
        level: Level,
        session: Option<&SessionId>,
        payload: serde_json::Value,
    ) {
        let guard = self.tx.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(tx) = guard.as_ref() {
            let mut event = Event::new(EventKind::Custom(kind.to_owned()), level, payload);
            if let Some(session) = session {
                event = event.with_session(session.clone());
            }
            let _ = tx.send(event);
        }
    }
}

#[async_trait]
impl Module for SqliteArtifacts {
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
