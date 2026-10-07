//! Implementacja `voice-dsp` (docs/modules/voice-dsp/SPEC.md, ADR 0011):
//! - **AEC: `sonora` (czysto-rustowy port WebRTC AEC3, BSD-3-Clause)** z referencją = własny
//!   strumień TTS (oś czasu odtworzenia z `voice-audio`, wyprzedzenie `reference_margin_ms`,
//!   korekta o skalibrowane opóźnienie pętli); alternatywą był `aec3` (mniej dojrzały) albo własny
//!   PBFDAF — niepotrzebny, AEC3 daje ERLE ≥ 15 dB w testach;
//! - NS: `nnnoiseless` (RNNoise w czystym Rust) + jego prawdopodobieństwo mowy;
//! - AGC: własny ([`agc::Agc`]), tryb szeptu;
//! - kalibracja pętli: korelacja wzajemna FFT (`rustfft`).
//!
//! Przetwarzanie: 48 kHz wewnętrznie, bloki 10 ms, wyjście 16 kHz mono (VAD/STT).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod agc;
pub mod calibrate;
mod pipeline;
pub mod reference;

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
pub use pipeline::DspPipeline;
pub use reference::DSP_RATE;
use voice_dsp_contract::{DspCfg, DspError, DspEvent};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł `voice-dsp`: fabryka potoków (jeden na strumień wejściowy) i publikacja zdarzeń.
pub struct VoiceDspModule {
    manifest: ModuleManifest,
    bus: Option<Arc<dyn EventBus>>,
}

impl VoiceDspModule {
    /// Moduł z manifestem.
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            bus: None,
        })
    }

    /// Nowy potok DSP.
    pub fn pipeline(&self, cfg: DspCfg) -> Result<DspPipeline, DspError> {
        DspPipeline::new(cfg)
    }

    /// Publikuje zdarzenia potoku (`Dsp::take_events`); zwraca liczbę opublikowanych.
    pub async fn publish(&self, events: &[DspEvent]) -> usize {
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
impl Module for VoiceDspModule {
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
