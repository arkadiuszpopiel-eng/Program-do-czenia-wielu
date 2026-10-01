//! Implementacja `voice-stt` (docs/modules/voice-stt/SPEC.md, ADR 0004): **sidecar `whisper-server`
//! z whisper.cpp** — proces na żądanie z argumentami z profilu (Vulkan/CUDA/CPU, model
//! `ggml-large-v3-turbo-q5_0.bin`), HTTP na `127.0.0.1` z losowym portem, bramka VAD przed wysłaniem,
//! dwa przebiegi (partial zachłanny + final z wiązką), fallback CPU po awarii / `ErrorDeviceLost`
//! bez utraty wypowiedzi, dzierżawa `model-residency`. Chmura: tylko typy (adapter w kolejnej fali).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod client;
mod engine;
pub mod sidecar;

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
pub use engine::{OWNER, WhisperStt};
pub use sidecar::{
    LaunchSpec, ProcessLauncher, Sidecar, SidecarBinaries, SidecarLauncher, WhisperServerConfig,
};
use voice_stt_contract::SttEvent;

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł `voice-stt`: publikacja zdarzeń silnika na magistralę.
pub struct VoiceSttModule {
    manifest: ModuleManifest,
    bus: Option<Arc<dyn EventBus>>,
}

impl VoiceSttModule {
    /// Moduł z manifestem.
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            bus: None,
        })
    }

    /// Publikuje zdarzenia (`Stt::take_events`); zwraca liczbę opublikowanych.
    pub async fn publish(&self, events: &[SttEvent]) -> usize {
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
impl Module for VoiceSttModule {
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
