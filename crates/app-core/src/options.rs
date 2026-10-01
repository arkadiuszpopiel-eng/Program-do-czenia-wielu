//! Opcje budowy `AppCore` (porty modułów do podmiany w testach i kolejnych falach); ścieżki
//! danych — `app_api::paths::AppPaths` (reeksport).

use std::sync::Arc;
use std::time::Duration;

use accounts_hub_contract::{CliProbe, SecretStore};
use agent_backends_contract::{AgentBackend, ApprovalSink};
use app_voice::VoiceEngineFactory;
use device_profile_contract::DeviceProfile as DeviceProfileService;
use platform_contract::{ClipboardPort, ExecPort, FsPort};
use providers_contract::ModelProvider;
use router_contract::RouteKind;
use voice_audio_contract::AudioIo;
use voice_tts_contract::Tts;

pub use app_api::paths::AppPaths;

use crate::events::DEFAULT_FRAME;
use crate::ports::{ApprovalWindow, BrainPort, BrokerPort, ShellPort, TransferPort, VoicePort};

/// Backend mostów CLI budowany nad kanałem zatwierdzeń aplikacji (testy: `agent-backends-fake`).
pub type BridgeFactory = Arc<dyn Fn(Arc<dyn ApprovalSink>) -> Arc<dyn AgentBackend> + Send + Sync>;

/// Opcje budowy. `None` w porcie = domyślna implementacja (produkcyjna albo „niepodłączony moduł").
pub struct AppOptions {
    /// Wersja aplikacji (do `app_bootstrap`).
    pub app_version: String,
    /// Długość klatki paczek zdarzeń (§14.7).
    pub frame: Duration,
    /// Okno cofnięcia usunięcia sesji (domyślnie z ustawień, 10 s).
    pub undo_window: Option<Duration>,
    /// Limit czekania agentki na zatwierdzenie (domyślnie z ustawień `agents.approval_timeout_s`).
    pub approval_timeout: Option<Duration>,
    /// Pobieranie kursu NBP (wyłączone → kurs zapasowy).
    pub fetch_fx: bool,
    /// Magazyn sekretów (`None` = Credential Manager; poza Windows — pamięć procesu).
    pub secrets: Option<Arc<dyn SecretStore>>,
    /// Profil urządzenia (`None` = detekcja sprzętu).
    pub device: Option<Arc<dyn DeviceProfileService>>,
    /// Wybór modelu (`None` = Router z dostawcami z `accounts-hub` i modelem lokalnym).
    pub brain: Option<Arc<dyn BrainPort>>,
    /// Dodatkowi dostawcy rejestrowani w Routerze (własne endpointy, testy na atrapach).
    pub providers: Vec<(Arc<dyn ModelProvider>, RouteKind)>,
    /// Import/eksport (`None` = moduł `transfer`).
    pub transfer: Option<Arc<dyn TransferPort>>,
    /// Głos (`None` = `voice-audio` + `voice-tts`).
    pub voice: Option<Arc<dyn VoicePort>>,
    /// Wejście/wyjście audio dla `voice-audio` (`None` = WASAPI; poza Windows — niedostępne).
    pub audio: Option<Arc<dyn AudioIo>>,
    /// Synteza mowy (`None` = sidecary Pocket TTS / Piper, jeśli zainstalowane).
    pub tts: Option<Arc<dyn Tts>>,
    /// Potok rozmowy głosowej (`None` = moduły `voice-*` z modelami z `AppPaths::models()`
    /// i sidecarami; bez nich — stan „głos niedostępny").
    pub voice_engine: Option<Arc<dyn VoiceEngineFactory>>,
    /// System plików narzędzi agentek i dziennika cofania (`None` = `platform-windows`).
    pub fs: Option<Arc<dyn FsPort>>,
    /// Wykonanie poleceń `shell_run` w Job Object (`None` = `platform-windows`).
    pub exec: Option<Arc<dyn ExecPort>>,
    /// Schowek narzędzi agentek (`None` = `platform-windows`).
    pub clipboard: Option<Arc<dyn ClipboardPort>>,
    /// Broker (`None` = `safety-broker` w procesie, tryb deweloperski).
    pub broker: Option<Arc<dyn BrokerPort>>,
    /// Okno zatwierdzeń Brokera (`None` = brak Broker-UI: prośby o zgodę są odrzucane).
    pub approval_window: Option<Arc<dyn ApprovalWindow>>,
    /// Powłoka (`None` = bez okien).
    pub shell: Option<Arc<dyn ShellPort>>,
    /// Mosty CLI (`None` = `agent-backends-impl` z wykrytymi CLI, przypięciami i zgodami).
    pub bridges: Option<BridgeFactory>,
    /// Wykrywanie CLI w PATH (`None` = `PATH`/`PATHEXT` procesu, `--version`).
    pub cli_probe: Option<Arc<dyn CliProbe>>,
    /// Po jakim czasie bez awarii start uznać za zdrowy (`updater::mark_good`).
    pub healthy_after: Duration,
    /// Zapis logów NDJSON z magistrali (`core-log`).
    pub file_logs: bool,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            frame: DEFAULT_FRAME,
            undo_window: None,
            approval_timeout: None,
            fetch_fx: true,
            secrets: None,
            device: None,
            brain: None,
            providers: Vec::new(),
            transfer: None,
            voice: None,
            audio: None,
            tts: None,
            voice_engine: None,
            fs: None,
            exec: None,
            clipboard: None,
            broker: None,
            approval_window: None,
            shell: None,
            bridges: None,
            cli_probe: None,
            healthy_after: Duration::from_secs(30),
            file_logs: true,
        }
    }
}
