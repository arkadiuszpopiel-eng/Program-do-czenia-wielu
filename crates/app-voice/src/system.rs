//! Produkcyjna fabryka potoku: `voice-*-impl` na urządzeniu audio aplikacji (WASAPI), STT
//! z sidecara `whisper-server` i modelu GGML z `AppPaths::models()/whisper`, VAD Silero (bez modelu
//! — detektor energii), synteza z silników aplikacji (Pocket TTS / Piper). Gdy brakuje STT albo
//! TTS — potok się nie składa, a UI pokazuje „głos niedostępny: pobierz modele…". Braki są
//! sprawdzane przy każdym zapytaniu (`missing`) — pliki pobrane w Ustawieniach działają bez restartu.
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
use model_residency_contract::Residency;
use personas_contract::builtin_personas;
use platform_contract::{Hotkey, HotkeyEvent, HotkeyId, HotkeyPort, PlatformError};
use scheduler_lite_contract::SchedulerLite;
use sessions_contract::KeyVault;
use voice_audio_contract::{AudioIo, StreamConfig};
use voice_cmd_contract::{Grammar, GrammarRecognizer};
use voice_dsp_contract::{DspCfg, NoiseSetting};
use voice_dsp_impl::DspPipeline;
use voice_pipeline_contract::{PipelineCfg, PipelineError, ReplySource};
use voice_pipeline_impl::{MonotonicClock, Pipeline, PipelineParts};
use voice_stt_contract::Stt;
use voice_stt_impl::fallback::GuardedStt;
use voice_stt_impl::qwen::QwenStt;
use voice_stt_impl::{ProcessLauncher, WhisperStt};
use voice_tts_contract::Tts;
use voice_turn_contract::TurnDetector;
use voice_turn_impl::{HeuristicTurnModel, PatienceTurnDetector, SmartTurnModel};
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
    /// Zegar potoku — w produkcji zegar urządzenia audio (ta sama podstawa co znaczniki ramek).
    pub(crate) clock: Arc<dyn voice_audio_contract::MediaClock>,
    noise: NoiseSetting,
    /// Miernik poziomu mowy dla interfejsu (awatary); bez niego wyjście jest nieopakowane.
    meter: Option<app_modules::speech_meter::SpeechMeter>,
    /// Ostatnio złożone rozpoznawanie ze strażnikiem (rozmowa albo dyktowanie) — do podglądu
    /// stanu bezpiecznika i telemetrii w Ustawieniach.
    pub(crate) guard: std::sync::Mutex<Option<Arc<GuardedStt>>>,
    /// Czy ostatnio złożony Qwen3-ASR liczy na karcie (`None` — jeszcze nie był składany).
    pub(crate) stt_on_gpu: std::sync::Mutex<Option<bool>>,
    /// Ustawienia rozmowy (Smart Turn, próg, najdłuższa cisza, silnik rozpoznawania).
    pub(crate) talk: crate::talk::TalkSettings,
}

/// Co minutę sprawdza, czy silnik Qwen3-ASR stoi bez użycia dłużej niż jego limit — wtedy go
/// zamyka i zwalnia pamięć karty. Zadanie kończy się samo, gdy silnik przestaje istnieć.
fn idle_unload(qwen: &Arc<QwenStt>) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    // Rozgrzewka: model wczytuje się w tle od startu rozmowy, nie przy pierwszym zdaniu.
    let warm = qwen.clone();
    runtime.spawn(async move {
        if let Err(e) = warm.warm().await {
            tracing::warn!(error = %e, "Qwen3-ASR: rozgrzewka nieudana (spróbuje przy pierwszej wypowiedzi)");
        }
    });
    let weak = Arc::downgrade(qwen);
    runtime.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let Some(engine) = weak.upgrade() else {
                break;
            };
            engine.unload_if_idle().await;
        }
    });
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
            clock: Arc::new(MonotonicClock::new()),
            noise: NoiseSetting::default(),
            meter: None,
            guard: std::sync::Mutex::new(None),
            stt_on_gpu: std::sync::Mutex::new(None),
            talk: crate::talk::TalkSettings::default(),
        }
    }

    /// Zegar w podstawie czasu urządzenia audio. Znaczniki ramek mikrofonu i pozycja odtwarzania
    /// przychodzą w czasie urządzenia (na Windows: QPC od startu systemu); z zegarem liczonym od
    /// startu potoku cisza po wypowiedzi wychodziła zerowa i tura nigdy się nie kończyła.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn voice_audio_contract::MediaClock>) -> Self {
        self.clock = clock;
        self
    }

    /// Ustawienia rozmowy śledzące konfigurację (czytane przy starcie rozmowy i dyktowania).
    #[must_use]
    pub fn with_talk(mut self, talk: crate::talk::TalkSettings) -> Self {
        self.talk = talk;
        self
    }

    /// Wspólne ustawienie redukcji szumów (Ustawienia → Głos); czytane przy starcie rozmowy.
    #[must_use]
    pub fn with_noise(mut self, noise: NoiseSetting) -> Self {
        self.noise = noise;
        self
    }

    /// Poziom mowy agentek do interfejsu — awatar rusza ustami w rytm głośnika.
    #[must_use]
    pub fn with_speech_meter(mut self, meter: app_modules::speech_meter::SpeechMeter) -> Self {
        self.meter = Some(meter);
        self
    }

    /// Sejf kluczy (Credential Manager) — klucz profilu głosu właściciela (bez sejfu
    /// rozpoznawanie głosu jest niedostępne).
    #[must_use]
    pub fn with_vault(mut self, vault: Arc<dyn KeyVault>) -> Self {
        self.vault = Some(vault);
        self
    }

    pub(crate) fn whisper_server(&self) -> PathBuf {
        app_modules::stt::server_cpu(&self.paths)
    }

    pub(crate) fn whisper_model(&self) -> Option<PathBuf> {
        app_modules::stt::model(&self.paths)
    }

    /// STT z sidecara `whisper-server` (rozmowa i dyktowanie): kompilacja CUDA ma pierwszeństwo,
    /// gdy jest zainstalowana (laptop z kartą NVIDIA), nieudany start → CPU.
    pub(crate) fn whisper(&self) -> Result<WhisperStt, PipelineError> {
        let (config, preferred) = app_modules::stt::whisper(&self.paths)
            .ok_or_else(|| component("stt", MISSING_STT_MODEL))?;
        let mut stt = WhisperStt::new(config, Arc::new(ProcessLauncher), preferred)
            .map_err(|e| component("stt", e))?;
        if let Some(r) = &self.residency {
            stt = stt.with_residency(r.clone());
        }
        Ok(stt)
    }

    /// Koniec wypowiedzi: model Smart Turn na dźwięku (z czasami strojonymi do rozmowy), gdy są
    /// jego pliki; inaczej model tekstowy z domyślną cierpliwością.
    pub(crate) fn turn(&self) -> Box<dyn TurnDetector> {
        let sidecar = self
            .talk
            .smart_turn()
            .then(|| app_modules::turn::smart_turn(&self.paths))
            .flatten();
        let Some(sidecar) = sidecar else {
            return Box::new(PatienceTurnDetector::new(HeuristicTurnModel));
        };
        let cfg = self.talk.turn_cfg();
        match PatienceTurnDetector::with_cfg(SmartTurnModel::sidecar(sidecar), cfg) {
            Ok(detector) => Box::new(detector),
            Err(e) => {
                tracing::warn!(error = %e, "Smart Turn niedostępny — koniec wypowiedzi z tekstu");
                Box::new(PatienceTurnDetector::new(HeuristicTurnModel))
            }
        }
    }

    /// Rozpoznawanie mowy dla rozmowy i dyktowania: Qwen3-ASR ze strażnikiem i Whisperem jako
    /// zapasem, gdy model Qwen3-ASR jest pobrany; inaczej sam Whisper.
    pub(crate) fn stt(&self) -> Result<Arc<dyn Stt>, PipelineError> {
        let whisper: Arc<dyn Stt> = Arc::new(self.whisper()?);
        if self.talk.whisper_only() {
            return Ok(whisper);
        }
        // Pomiar wolnej pamięci karty: gdy zajął ją model rozmowy, Qwen3-ASR liczy na procesorze.
        let free = app_modules::gpu_mem::nvidia_free_mb();
        let Some(config) = app_modules::stt::qwen(&self.paths, free) else {
            return Ok(whisper);
        };
        let on_gpu = config.gpu;
        tracing::info!(on_gpu, free_vram_mb = ?free, "Qwen3-ASR: wybór karta / procesor");
        match QwenStt::new(config, Arc::new(ProcessLauncher)) {
            Ok(qwen) => {
                let qwen = Arc::new(qwen);
                idle_unload(&qwen);
                *self
                    .stt_on_gpu
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(on_gpu);
                let guarded = Arc::new(GuardedStt::new(qwen, whisper));
                *self
                    .guard
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(guarded.clone());
                Ok(guarded)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Qwen3-ASR niedostępny — rozpoznaje Whisper");
                Ok(whisper)
            }
        }
    }

    /// Silnik TTS gotowy teraz: produkcyjny (`app_modules::tts::engines`) wykrywa Pipera i Pocket
    /// TTS przy każdym sprawdzeniu, więc silnik pobrany w Ustawieniach działa bez restartu Alfy.
    pub(crate) fn tts_ready(&self) -> Option<Arc<dyn Tts>> {
        app_modules::tts::ready(self.tts.as_ref())
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
        if self.tts_ready().is_none() {
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
            .tts_ready()
            .ok_or_else(|| component("tts", MISSING_TTS))?;
        let stt = self.stt()?;
        // Rozgrzewka głosu w tle: proces mowy startuje teraz, a nie przy pierwszej odpowiedzi.
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let voice = tts.clone();
            runtime.spawn(async move {
                let _ = voice.warm(&personas_contract::PersonaId::alfa()).await;
            });
        }
        let output = self
            .audio
            .open_output(None, &StreamConfig::output_default())
            .map_err(|e| component("speaker", e))?;
        let output: Box<dyn voice_audio_contract::OutputStream> = match &self.meter {
            Some(meter) => Box::new(app_modules::speech_meter::MeteredOutput::new(
                output,
                meter.clone(),
            )),
            None => output,
        };
        let dsp = DspPipeline::new(DspCfg::default().with_noise(self.noise.get()))
            .map_err(|e| component("dsp", e))?;
        let parts = PipelineParts {
            clock: self.clock.clone(),
            audio: self.audio.clone(),
            output,
            dsp: Box::new(dsp),
            vad: Box::new(self.vad()?),
            stt,
            turn: self.turn(),
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
