//! Implementacja `voice-speaker` (docs/modules/voice-speaker/SPEC.md): rdzeń `SpeakerEngine`
//! z kontraktu + embedding ECAPA/WeSpeaker z ONNX przez `tract-onnx` ([`OnnxSpeakerModel`],
//! manifest z SHA-256) + profil zaszyfrowany lokalnie ([`EncryptedFileStore`]: XChaCha20-Poly1305,
//! klucz z sejfu — Credential Manager; usunięcie = crypto-shredding) + bramka właściciela dla słów
//! wywoławczych ([`SpeakerOwnerCheck`]) + runner EER ([`eval`], bin `alfa-speaker-eval`).
//! Model **nie jest w repo** (README: skąd wziąć i licencje).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod eval;
mod model;
mod store;

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use sessions_contract::KeyVault;
use voice_speaker_contract::{
    Decision, SpeakerCfg, SpeakerEngine, SpeakerError, SpeakerEvent, SpeakerVerifier,
};
use voice_wake_contract::OwnerCheck;

pub use model::{OnnxSpeakerModel, SPEAKER_MANIFEST_FORMAT, SpeakerManifest, sha256_hex};
pub use store::{EncryptedFileStore, KEY_NAME, MAGIC};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Weryfikator produkcyjny.
pub type SpeakerService = SpeakerEngine<OnnxSpeakerModel>;

/// Składa weryfikator: model z manifestu (hash sprawdzany), profil w zaszyfrowanym pliku.
pub fn open_speaker(
    model_manifest: &Path,
    profile_path: &Path,
    vault: Arc<dyn KeyVault>,
    cfg: SpeakerCfg,
) -> Result<SpeakerService, SpeakerError> {
    let model = OnnxSpeakerModel::load(model_manifest)?;
    let store = EncryptedFileStore::new(profile_path, vault);
    SpeakerEngine::new(model, Box::new(store), cfg)
}

/// Bramka właściciela dla słów wywoławczych (`voice-wake` v1, `owner_gate`): fraza musi być
/// co najmniej „prawdopodobnie właściciela” (próg standardowy); brak profilu / błąd → `None`
/// (nasłuch traktuje to jak odmowę — fail-closed).
pub struct SpeakerOwnerCheck(pub Arc<dyn SpeakerVerifier>);

impl OwnerCheck for SpeakerOwnerCheck {
    fn is_owner(&mut self, audio: &[f32]) -> Option<bool> {
        self.0
            .verify(audio)
            .ok()
            .map(|v| v.decision != Decision::Rejected)
    }
}

/// Moduł `voice-speaker`: publikacja zdarzeń (bez audio i embeddingu).
pub struct VoiceSpeakerModule {
    manifest: ModuleManifest,
    bus: Option<Arc<dyn EventBus>>,
}

impl VoiceSpeakerModule {
    /// Moduł z manifestem.
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            bus: None,
        })
    }

    /// Publikuje zdarzenia; zwraca liczbę opublikowanych.
    pub async fn publish(&self, events: &[SpeakerEvent]) -> usize {
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
impl Module for VoiceSpeakerModule {
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
