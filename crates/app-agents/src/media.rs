//! F6: wizja i multimedia agentek (grupy ról `vision`, `media`):
//! - `tools-vision` — OCR przez `Windows.Media.Ocr` (`platform-windows-ocr-impl`) na zrzutach
//!   z portów GUI (ten sam strażnik okien Alfy/Brokera co computer use), opis obrazu przez Router
//!   „GUI/wizja”: zwykła sesja — rdzeń hybrydowy, prywatna/„tylko lokalnie” — wyłącznie lokalny
//!   (prywatność z katalogu sesji; sesja nieznana = tylko lokalnie);
//! - `tools-media` — nagłówki (`lib-media`, odczyt fragmentami z dysku), `ffmpeg` jako sidecar
//!   `sidecars\ffmpeg` (instalacja ręczna, pozycja katalogu modeli „do potwierdzenia”) w Job Object
//!   tego samego `ExecPort` co kill-switch, odtwarzanie przez `voice-audio` w kolejce mówienia.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use app_api::AppPaths;
use app_gui::GuiPorts;
use app_modules::route::Routers;
use async_trait::async_trait;
use compliance_contract::{DenyLists, PathEnv};
use core_bus_contract::EventBus;
use platform_contract::{DesktopPort, ExecPort, FsPort, ScreenCapturePort};
use providers_contract::ModelProvider;
use router_contract::TaskClass;
use router_impl::RoutedProvider;
use safety_broker_contract::Broker;
use scheduler_lite_contract::SchedulerLite;
use sessions_contract::{PrivacyTag, SessionCatalog, SessionId};
use tools_common_contract::{Tool, Toolset};
use tools_media_contract::{
    AudioClip, AudioPlayer, CancellationToken, ConvertError, ConvertJob, MediaToolsConfig,
    PlayError, PlayTicket, Transcoder,
};
use tools_media_impl::{
    FfmpegConfig, FfmpegTranscoder, MediaTools, MediaToolsDeps, PlayerConfig, SpeakerPlayer,
};
use tools_vision_contract::{PrivacyLookup, VisionPrivacy, VisionToolsConfig};
use tools_vision_impl::{RouterDescriber, VisionTools, VisionToolsDeps};
use undo_journal_contract::UndoJournal;
use voice_audio_contract::AudioIo;
use voice_audio_impl::VoiceAudioModule;

/// Prywatność opisu obrazu z katalogu sesji (sesja nieznana albo błąd → tylko lokalnie).
struct CatalogPrivacy(Arc<dyn SessionCatalog>);

impl PrivacyLookup for CatalogPrivacy {
    fn vision_privacy(&self, session: &str) -> VisionPrivacy {
        match self.0.session(&SessionId::new(session)) {
            Ok(meta) if meta.privacy == PrivacyTag::Normal => VisionPrivacy::Normal,
            _ => VisionPrivacy::LocalOnly,
        }
    }
}

/// Bez wyjścia audio (moduł `voice-audio` niepodłączony).
struct NoSpeaker;

#[async_trait]
impl AudioPlayer for NoSpeaker {
    async fn play(&self, _: AudioClip, _: CancellationToken) -> Result<PlayTicket, PlayError> {
        Err(PlayError::Unavailable(
            "moduł voice-audio niepodłączony".into(),
        ))
    }

    fn stop_all(&self) -> usize {
        0
    }
}

/// Bez portu uruchamiania (Broker niepodłączony) — konwersje niedostępne.
struct NoExec;

impl Transcoder for NoExec {
    fn available(&self) -> Result<(), ConvertError> {
        Err(ConvertError::NotInstalled(
            "brak uruchamiania procesów w Job Object".into(),
        ))
    }

    fn convert(&self, _: &ConvertJob, _: Arc<AtomicBool>) -> Result<Vec<u8>, ConvertError> {
        self.available().map(|()| Vec::new())
    }
}

/// Routery klasy „GUI/wizja”: (hybrydowy, lokalny).
type VisionRoutes = (
    Option<Arc<dyn ModelProvider>>,
    Option<Arc<dyn ModelProvider>>,
);

/// Porty narzędzi wizji i multimediów (składane w `app-core` obok narzędzi GUI).
#[derive(Clone)]
pub struct MediaPorts {
    desktop: Arc<dyn DesktopPort>,
    capture: Arc<dyn ScreenCapturePort>,
    privacy: Arc<dyn PrivacyLookup>,
    scheduler: Arc<dyn SchedulerLite>,
    env: PathEnv,
    deny: DenyLists,
    ffmpeg: FfmpegConfig,
    vision: VisionRoutes,
    audio: Option<Arc<dyn AudioIo>>,
    exec: Option<Arc<dyn ExecPort>>,
}

impl std::fmt::Debug for MediaPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaPorts")
            .field("ffmpeg", &self.ffmpeg)
            .field("audio", &self.audio.is_some())
            .finish_non_exhaustive()
    }
}

/// Plik `ffmpeg` w katalogu sidecarów (`sidecars\ffmpeg\ffmpeg.exe`).
pub fn ffmpeg_path(paths: &AppPaths) -> PathBuf {
    let exe = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    paths.sidecars().join("ffmpeg").join(exe)
}

impl MediaPorts {
    /// Porty GUI (zrzuty ze strażnikiem okien), katalog sesji, kolejka mówienia, ścieżki Alfy.
    pub fn new(
        gui: &GuiPorts,
        sessions: Arc<dyn SessionCatalog>,
        scheduler: Arc<dyn SchedulerLite>,
        paths: &AppPaths,
    ) -> Self {
        Self {
            desktop: gui.desktop.clone(),
            capture: gui.capture.clone(),
            privacy: Arc::new(CatalogPrivacy(sessions)),
            scheduler,
            env: app_modules::broker::path_env_for(&paths.user_root).1,
            deny: app_modules::workdir::tool_deny_lists(paths),
            ffmpeg: FfmpegConfig::new(ffmpeg_path(paths), paths.local.join("media-work")),
            vision: (None, None),
            audio: None,
            exec: None,
        }
    }

    /// Router (klasa „GUI/wizja”: hybrydowy i lokalny) i wyjście audio.
    #[must_use]
    pub fn with(
        mut self,
        routers: Option<&Routers>,
        audio: Option<&Arc<VoiceAudioModule>>,
    ) -> Self {
        let routed = |core| {
            Arc::new(RoutedProvider::new(core, TaskClass::GuiVision)) as Arc<dyn ModelProvider>
        };
        self.vision = match routers {
            Some(r) => (
                Some(routed(r.hybrid.clone())),
                Some(routed(r.local.clone())),
            ),
            None => (None, None),
        };
        self.audio = audio.map(|a| a.io());
        self
    }

    /// Uruchamianie procesów (ffmpeg) w Job Object — ta sama instancja co kill-switch Brokera.
    #[must_use]
    pub fn exec(mut self, exec: Option<Arc<dyn ExecPort>>) -> Self {
        self.exec = exec;
        self
    }

    /// Narzędzia `vision_*` i `media_*` nad Brokerem (rejestr kart), dziennikiem cofania i FS.
    pub fn tools(
        &self,
        broker: Arc<dyn Broker>,
        journal: Arc<dyn UndoJournal>,
        fs: Arc<dyn FsPort>,
        bus: &Arc<dyn EventBus>,
    ) -> Vec<Arc<dyn Tool>> {
        let config = VisionToolsConfig::default();
        let (normal, local) = self.vision.clone();
        let mut tools = VisionTools::new(VisionToolsDeps {
            desktop: self.desktop.clone(),
            capture: self.capture.clone(),
            ocr: Arc::new(platform_windows_ocr_impl::WinOcr::new(config.max_pixels)),
            describer: Arc::new(RouterDescriber::new(
                normal,
                local,
                router_contract::AUTO_MODEL,
            )),
            privacy: self.privacy.clone(),
            files: Arc::new(lib_media::StdFiles),
            broker: broker.clone(),
            env: self.env.clone(),
            deny: self.deny.clone(),
            config,
            bus: Some(bus.clone()),
        })
        .tools();
        let player: Arc<dyn AudioPlayer> = match &self.audio {
            Some(io) => Arc::new(SpeakerPlayer::new(
                io.clone(),
                self.scheduler.clone(),
                PlayerConfig::default(),
            )),
            None => Arc::new(NoSpeaker),
        };
        let transcoder: Arc<dyn Transcoder> = match &self.exec {
            Some(exec) => Arc::new(FfmpegTranscoder::new(exec.clone(), self.ffmpeg.clone())),
            None => Arc::new(NoExec),
        };
        tools.extend(
            MediaTools::new(MediaToolsDeps {
                fs,
                files: Arc::new(lib_media::StdFiles),
                journal,
                transcoder,
                player,
                broker,
                env: self.env.clone(),
                deny: self.deny.clone(),
                config: MediaToolsConfig::default(),
                bus: Some(bus.clone()),
            })
            .tools(),
        );
        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallbacks_refuse_with_a_reason() {
        let t = NoExec;
        assert!(matches!(t.available(), Err(ConvertError::NotInstalled(_))));
        assert_eq!(NoSpeaker.stop_all(), 0);
        let paths = AppPaths::under(std::path::Path::new("/alfa"));
        assert!(ffmpeg_path(&paths).starts_with(paths.sidecars().join("ffmpeg")));
    }
}
