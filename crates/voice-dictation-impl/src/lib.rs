//! Implementacja `voice-dictation` (docs/modules/voice-dictation/SPEC.md): [`DictationService`]
//! (cel = okno z chwili startu, `InputPort` w porcjach atomowych, odmowa dla okien chronionych,
//! administratora i pól haseł — UIA, fail-closed; „cofnij to” przez Backspace) i
//! [`DictationRunner`] (mikrofon z dzierżawą `scheduler-lite` → VAD → STT → usługa; PTT
//! i przełącznik). Dyktowany tekst nie trafia do zdarzeń, logów ani pamięci.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod focus;
mod runner;
mod service;

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use voice_dictation_contract::DictationEvent;

pub use focus::{focused_node, password_risk};
pub use runner::{DictationAudio, DictationRunner};
pub use service::{CHUNK_UNITS, DictationPorts, DictationService, chunks};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł `voice-dictation`: publikacja zdarzeń (bez treści).
pub struct VoiceDictationModule {
    manifest: ModuleManifest,
    bus: Option<Arc<dyn EventBus>>,
}

impl VoiceDictationModule {
    /// Moduł z manifestem.
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            bus: None,
        })
    }

    /// Publikuje zdarzenia; zwraca liczbę opublikowanych.
    pub async fn publish(&self, events: &[DictationEvent]) -> usize {
        let Some(bus) = &self.bus else {
            return 0;
        };
        let mut n = 0;
        for e in events {
            if bus.publish(e.to_bus_event()).await.is_ok() {
                n += 1;
            }
        }
        n
    }
}

#[async_trait]
impl Module for VoiceDictationModule {
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
        if self.bus.is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::NotStarted
        }
    }
}
