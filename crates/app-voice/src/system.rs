//! Produkcyjna fabryka potoku: `voice-*-impl` na urządzeniu audio aplikacji (WASAPI), STT
//! z sidecara `whisper-server` i modelu GGML z `AppPaths::models()/whisper`, VAD Silero (bez modelu
//! — detektor energii), synteza z silników aplikacji (Pocket TTS / Piper). Gdy brakuje STT albo
//! TTS — potok się nie składa, a UI pokazuje „głos niedostępny: pobierz modele…".
//!
//! Skróty globalne (PTT `WH_KEYBOARD_LL`, przełącznik) obsługuje powłoka i przekazuje komendami
//! `voice_ptt` / `voice_set_mic_enabled` — `voice-wake` dostaje port skrótów bez rejestracji.
//! Automat aktywacji ma zawsze skonfigurowane słowa wywoławcze (z person); czy nasłuch działa,
//! decyduje uzbrojenie (`features::wake` — tylko po jawnym włączeniu). Składniki F5: `system_f5.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use app_api::paths::AppPaths;
use core_bus_contract::EventBus;
use device_profile_contract::Backend;
use model_residency_contract::Residency;
use personas_contract::builtin_personas;
use platform_contract::{Hotkey, HotkeyEvent, HotkeyId, HotkeyPort, PlatformError};
use scheduler_lite_contract::SchedulerLite;
use sessions_contract::KeyVault;
use voice_audio_contract::{AudioIo, StreamConfig};
use voice_cmd_contract::{Grammar, GrammarRecognizer};
use voice_dsp_contract::DspCfg;
use voice_dsp_impl::DspPipeline;
use voice_pipeline_contract::{PipelineCfg, PipelineError, ReplySource};
use voice_pipeline_impl::{MonotonicClock, Pipeline, PipelineParts};
use voice_stt_impl::{ProcessLauncher, SidecarBinaries, WhisperServerConfig, WhisperStt};
use voice_tts_contract::Tts;
use voice_turn_impl::{HeuristicTurnModel, PatienceTurnDetector};
use voice_vad_contract::VadCfg;
use voice_vad_impl::{HashPolicy, SileroModel, SileroVad};
use voice_wake_contract::{Wake, WakeCfg, WakeWordCfg};
use voice_wake_impl::WakeService;

use crate::engine::{IntervalPacer, VoiceEngine, VoiceEngineFactory};
use crate::features::FeatureFactory;

/// Brak sidecara STT.
pub const MISSING_STT_SIDECAR: &str = "whisper-server";
/// Brak modelu STT.
pub const MISSING_STT_MODEL: &str = "model STT (whisper GGML)";
/// Brak silnika TTS.
pub const MISSING_TTS: &str = "silnik TTS (Pocket TTS / Piper)";

/// Port skrótów bez rejestracji (skróty głosu obsługuje powłoka).
struct ShellHotkeys;

impl HotkeyPort for ShellHotkeys {
    fn register(&self, _hotkey: Hotkey) -> Result<HotkeyId, PlatformError> {
        Ok(HotkeyId(0))
    }
    fn unregister(&self, _id: HotkeyId) -> Result<(), PlatformError> {
        Ok(())
    }
    fn drain_events(&self) -> Vec<HotkeyEvent> {
        Vec::new()
    }
}

/// Fabryka produkcyjna.
pub struct SystemVoice {
    pub(crate) paths: AppPaths,
    pub(crate) audio: Arc<dyn AudioIo>,
    pub(crate) tts: Option<Arc<dyn Tts>>,
    pub(crate) scheduler: Arc<dyn SchedulerLite>,
    residency: Option<Arc<dyn Residency>>,
    pub(crate) vault: Option<Arc<dyn KeyVault>>,
    pub(crate) clock: MonotonicClock,
}

pub(crate) fn component(component: &str, e: impl std::fmt::Display) -> PipelineError {
    PipelineError::Component {
        component: component.to_owned(),
        reason: e.to_string(),
    }
}

/// Pierwszy plik w katalogu spełniający warunek (nazwy rosnąco — deterministycznie).
pub(crate) fn first_file(dir: &Path, pick: impl Fn(&str) -> bool) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.file_name().and_then(|n| n.to_str()).is_some_and(&pick))
        .collect();
    found.sort();
    found.into_iter().next()
}

/// Pierwszy model GGML w katalogu.
fn ggml_model(dir: &Path) -> Option<PathBuf> {
    first_file(dir, |n| n.starts_with("ggml-") && n.ends_with(".bin"))
}

/// Automat aktywacji: bez skrótów (obsługuje je powłoka), ze słowami wywoławczymi z person
/// (wykrycia przychodzą tylko z uzbrojonego nasłuchu).
fn wake_service() -> Result<WakeService, PipelineError> {
    let mut wake = WakeService::new(Arc::new(ShellHotkeys), builtin_personas(), None);
    wake.configure(WakeCfg {
        ptt_key: None,
        toggle_key: None,
        name_addressing: true,
        wake_words: Some(WakeWordCfg::from_personas(
            &builtin_personas(),
            crate::features::wake::DEFAULT_THRESHOLD,
        )),
    })
    .map_err(|e| component("wake", e))?;
    Ok(wake)
}

impl SystemVoice {
    /// Fabryka nad urządzeniem audio, silnikiem TTS, schedulerem i rezydencją aplikacji.
    pub fn new(
        paths: AppPaths,
        audio: Arc<dyn AudioIo>,
        tts: Option<Arc<dyn Tts>>,
        scheduler: Arc<dyn SchedulerLite>,
        residency: Option<Arc<dyn Residency>>,
    ) -> Self {
        Self {
            paths,
            audio,
            tts,
            scheduler,
            residency,
            vault: None,
            clock: MonotonicClock::new(),
        }
    }

    /// Sejf kluczy (Credential Manager) — klucz profilu głosu właściciela (bez sejfu
    /// rozpoznawanie głosu jest niedostępne).
    #[must_use]
    pub fn with_vault(mut self, vault: Arc<dyn KeyVault>) -> Self {
        self.vault = Some(vault);
        self
    }

    fn whisper_server(&self) -> PathBuf {
        self.paths.sidecar("whisper", "whisper-server")
    }

    fn whisper_model(&self) -> Option<PathBuf> {
        ggml_model(&self.paths.models().join("whisper"))
    }

    /// STT z sidecara `whisper-server` (rozmowa i dyktowanie).
    pub(crate) fn whisper(&self) -> Result<WhisperStt, PipelineError> {
        let model = self
            .whisper_model()
            .ok_or_else(|| component("stt", MISSING_STT_MODEL))?;
        let binaries = SidecarBinaries {
            vulkan: None,
            cuda: None,
            cpu: self.whisper_server(),
        };
        let mut stt = WhisperStt::new(
            WhisperServerConfig::new(binaries, model),
            Arc::new(ProcessLauncher),
            Backend::Cpu,
        )
        .map_err(|e| component("stt", e))?;
        if let Some(r) = &self.residency {
            stt = stt.with_residency(r.clone());
        }
        Ok(stt)
    }

    /// VAD Silero (bez modelu — detektor energii).
    pub(crate) fn vad(&self) -> Result<SileroVad, PipelineError> {
        let path = self.paths.models().join("silero").join("silero_vad.onnx");
        let model = if path.is_file() {
            match SileroModel::load(&path, HashPolicy::KnownOnly) {
                Ok(m) => Some(m),
                Err(e) => {
                    tracing::warn!(error = %e, "VAD Silero niedostępny — detektor energii");
                    None
                }
            }
        } else {
            None
        };
        SileroVad::new(VadCfg::default(), model).map_err(|e| component("vad", e))
    }
}

impl VoiceEngineFactory for SystemVoice {
    fn missing(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.whisper_server().is_file() {
            out.push(MISSING_STT_SIDECAR.to_owned());
        }
        if self.whisper_model().is_none() {
            out.push(MISSING_STT_MODEL.to_owned());
        }
        if self.tts.is_none() {
            out.push(MISSING_TTS.to_owned());
        }
        out
    }

    fn build(
        &self,
        reply: Arc<dyn ReplySource>,
        bus: Arc<dyn EventBus>,
        cfg: PipelineCfg,
    ) -> Result<VoiceEngine, PipelineError> {
        let tts = self
            .tts
            .clone()
            .ok_or_else(|| component("tts", MISSING_TTS))?;
        let stt = self.whisper()?;
        let output = self
            .audio
            .open_output(None, &StreamConfig::output_default())
            .map_err(|e| component("speaker", e))?;
        let dsp = DspPipeline::new(DspCfg::default()).map_err(|e| component("dsp", e))?;
        let parts = PipelineParts {
            clock: Arc::new(MonotonicClock::new()),
            audio: self.audio.clone(),
            output,
            dsp: Box::new(dsp),
            vad: Box::new(self.vad()?),
            stt: Arc::new(stt),
            turn: Box::new(PatienceTurnDetector::new(HeuristicTurnModel)),
            commands: Arc::new(GrammarRecognizer::new(Grammar::default_pl_en())),
            dialog: Box::new(voice_dialog_contract::default_machine()),
            wake: Box::new(wake_service()?),
            persona: Arc::new(voice_persona_impl::PersonaService::new()),
            tts,
            reply,
            scheduler: self.scheduler.clone(),
            residency: self.residency.clone(),
            bus: Some(bus),
        };
        let tick = Duration::from_millis(u64::from(cfg.tick_ms));
        let pipeline = Pipeline::new(parts, cfg)?;
        Ok(VoiceEngine {
            pipeline: Box::new(pipeline),
            pacer: Box::new(IntervalPacer::new(tick)),
        })
    }

    fn features(&self) -> Option<&dyn FeatureFactory> {
        Some(self)
    }
}
