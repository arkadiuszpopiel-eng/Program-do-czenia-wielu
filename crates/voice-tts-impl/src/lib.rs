//! Implementacja `voice-tts` (docs/modules/voice-tts/SPEC.md, ADR 0011): sidecary **Pocket TTS PL**
//! (trwały proces, JSON-lines po stdio) i **Piper** `pl_PL` (proces na zdanie), **głosy v0 bez
//! kluczy** — 2 mówczynie bazowe × wysokość/tempo ([`voice_mod`]: WSOLA + resampling), łańcuch
//! fallback per agentka, cache fraz stałych, TTFB jako zdarzenie, dzierżawa `model-residency`.
//! Chmurowe TTS: tylko typy (adapter w kolejnej fali); w sesji prywatnej pomijane.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod cache;
pub mod engines;
mod service;
pub mod voice_mod;

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use model_residency_contract::{
    LeaseId, LeaseRequest, ModelRole, Placement, Priority, Residency, ResidencyError,
};
pub use service::TtsService;
use voice_tts_contract::TtsEvent;

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Dzierżawa modelu TTS na CPU (Pocket TTS PL ≈ 0,5–1,5 GB RAM), zwalniana przy `drop`.
pub struct TtsLease {
    residency: Arc<dyn Residency>,
    id: LeaseId,
}

impl TtsLease {
    /// Zgłasza model w `model-residency` (CPU, priorytet głosu, wyładowanie po bezczynności).
    pub fn acquire(
        residency: Arc<dyn Residency>,
        model: &str,
        ram_mb: u32,
        idle_unload_ms: u64,
    ) -> Result<Self, ResidencyError> {
        let request = LeaseRequest {
            owner: "voice-tts".into(),
            model: model.into(),
            role: ModelRole::Tts,
            priority: Priority::VoiceRt,
            placement: Placement::CpuOnly,
            vram_mb: 0,
            ram_mb: 0,
            cpu_ram_mb: ram_mb,
            idle_unload_ms,
        };
        let grant = residency.acquire(request)?;
        Ok(Self {
            residency,
            id: grant.lease.id,
        })
    }

    /// Identyfikator.
    pub fn id(&self) -> LeaseId {
        self.id
    }
}

impl Drop for TtsLease {
    fn drop(&mut self) {
        let _ = self.residency.release(self.id);
    }
}

/// Moduł `voice-tts`: publikacja zdarzeń.
pub struct VoiceTtsModule {
    manifest: ModuleManifest,
    bus: Option<Arc<dyn EventBus>>,
}

impl VoiceTtsModule {
    /// Moduł z manifestem.
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            bus: None,
        })
    }

    /// Publikuje zdarzenia (`Tts::take_events`).
    pub async fn publish(&self, events: &[TtsEvent]) -> usize {
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
impl Module for VoiceTtsModule {
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
