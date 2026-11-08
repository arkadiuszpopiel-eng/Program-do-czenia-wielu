//! Implementacja `voice-vad` (docs/modules/voice-vad/SPEC.md): **Silero VAD przez `tract-onnx`**
//! (czysty Rust, bez pobierania binariów ONNX Runtime — `ort` odrzucony: pobiera ORT z sieci przy
//! budowie, a `tract` obsługuje model Silero 6.x z operatorami `If`), detektor energii jako zapas,
//! dzierżawa w `model-residency`, zdarzenia `voice.vad.model.*`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod detector;
mod inline;
pub mod residency;
pub mod silero;

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
pub use detector::SileroVad;
use model_residency_contract::Residency;
use residency::{LeaseGuard, vad_lease_request};
pub use silero::{HashPolicy, KNOWN_MODELS, SileroModel};
use voice_vad_contract::{
    EVENT_MODEL_LOADED, EVENT_MODEL_UNLOADED, VadCfg, VadEngine, VadError, VadEvent, event_kind,
};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");
/// Zmienna środowiskowa ze ścieżką modelu (testy z prawdziwym modelem).
pub const MODEL_ENV: &str = "ALFA_SILERO_VAD";

/// Moduł `voice-vad`: tworzy detektory (z modelem i dzierżawą) i publikuje zdarzenia.
pub struct VoiceVadModule {
    manifest: ModuleManifest,
    bus: Option<Arc<dyn EventBus>>,
    residency: Option<Arc<dyn Residency>>,
}

impl VoiceVadModule {
    /// Moduł bez zarządcy rezydencji.
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            bus: None,
            residency: None,
        })
    }

    /// Zgłasza model do `model-residency`.
    #[must_use]
    pub fn with_residency(mut self, residency: Arc<dyn Residency>) -> Self {
        self.residency = Some(residency);
        self
    }

    /// Tworzy VAD. Silero: ładuje `model_path` (hash wg `policy`), bierze dzierżawę i publikuje
    /// `voice.vad.model.loaded`; błąd modelu → detektor energii (zdarzenie z powodem).
    pub async fn create(
        &self,
        cfg: VadCfg,
        model_path: Option<&Path>,
        policy: HashPolicy,
    ) -> Result<SileroVad, VadError> {
        let wants_model = cfg.engine != VadEngine::Energy;
        let model = match (wants_model, model_path) {
            (true, Some(path)) => match SileroModel::load(path, policy) {
                Ok(m) => Some(m),
                Err(e) => {
                    self.emit(
                        EVENT_MODEL_UNLOADED,
                        serde_json::json!({ "reason": e.to_string() }),
                    )
                    .await;
                    None
                }
            },
            _ => None,
        };
        let lease = match (&model, &self.residency) {
            (Some(m), Some(r)) => Some(
                LeaseGuard::acquire(Arc::clone(r), vad_lease_request(m.name()))
                    .map_err(|e| VadError::Model(format!("rezydencja: {e}")))?,
            ),
            _ => None,
        };
        if let Some(m) = &model {
            self.emit(EVENT_MODEL_LOADED, serde_json::json!({ "model": m.name() }))
                .await;
        }
        Ok(SileroVad::new(cfg, model)?.with_lease(lease))
    }

    /// Publikuje zdarzenia VAD.
    pub async fn publish(&self, events: &[VadEvent]) -> usize {
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

    async fn emit(&self, name: &str, payload: serde_json::Value) {
        if let Some(bus) = &self.bus {
            let _ = bus
                .publish(Event::new(event_kind(name), Level::Info, payload))
                .await;
        }
    }
}

#[async_trait]
impl Module for VoiceVadModule {
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
