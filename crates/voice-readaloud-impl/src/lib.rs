//! Implementacja `voice-readaloud` (docs/modules/voice-readaloud/SPEC.md): [`UiaTextSource`]
//! (UIA `TextPattern` tylko do odczytu, zaznaczenie przez `SelectionReader`, zapas Ctrl+C
//! z przywróceniem schowka; odmowa dla pól haseł i okien Alfy/Brokera) i [`ReadAloudService`]
//! (zdania → `voice-tts` głosem agentki, lokalnie → wyjście audio, głośnik w `scheduler-lite`).
//! Czytana treść jest niezaufana: nie trafia do zdarzeń, pamięci ani modelu bez zgody.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod service;
mod source;

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use voice_readaloud_contract::ReadAloudEvent;

pub use service::{ReadAloudParts, ReadAloudService, UTTERANCE_BASE};
pub use source::{CopyFallback, UiaTextSource};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł `voice-readaloud`: publikacja zdarzeń (bez treści).
pub struct VoiceReadAloudModule {
    manifest: ModuleManifest,
    bus: Option<Arc<dyn EventBus>>,
}

impl VoiceReadAloudModule {
    /// Moduł z manifestem.
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            bus: None,
        })
    }

    /// Publikuje zdarzenia; zwraca liczbę opublikowanych.
    pub async fn publish(&self, events: &[ReadAloudEvent]) -> usize {
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
impl Module for VoiceReadAloudModule {
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
