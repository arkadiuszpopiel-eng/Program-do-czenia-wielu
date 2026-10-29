//! Narzędzia agentek (`tools-fs/shell/clipboard` nad Brokerem w procesie i dziennikiem cofania)
//! i port głosu z potokiem rozmowy — składane po zbudowaniu modułów (jedno miejsce kompozycji).

use std::sync::Arc;

use app_agents::{AgentTools, ShellToolsConfig, TicketLog, ToolsDeps};
use app_modules::broker::path_env_for;
use app_modules::secrets::StoreKeyVault;
use app_voice::{DesktopDeps, FeatureDeps, PipelineVoice, SystemVoice, VoiceEngineFactory};
use core_bus_contract::EventBus;
use safety_broker_contract::Broker;
use voice_pipeline_contract::PipelineCfg;

use super::Extra;
use super::extra::platform_fs;
use crate::options::{AppOptions, AppPaths};
use crate::ports::{VoicePort, VoiceUnavailable};

/// Zasoby wyłączne i obsada dla startu przebiegów v1; katalog sesji (prywatność opisu obrazu).
pub(crate) type Launchers = (
    Arc<dyn scheduler_lite_contract::SchedulerLite>,
    Arc<dyn personas_contract::Personas>,
    Arc<sessions_impl::SqliteSessions>,
);

/// Narzędzia agentek i rejestr próśb o zatwierdzenie (typ silnika czatu).
pub(crate) use app_chat::AgentStack;

impl Extra {
    /// Narzędzia agentek — tylko z Brokerem i dziennikiem cofania (bez nich agentki odpowiadają
    /// bez narzędzi; żadna akcja nie omija Brokera).
    pub(crate) fn agent_stack(
        &self,
        (options, paths): (&AppOptions, &AppPaths),
        bus: &Arc<dyn EventBus>,
        mut extra: Vec<Arc<dyn tools_common_contract::Tool>>,
        (gui, monitor): (&app_gui::GuiPorts, &Arc<app_gui::GuiMonitor>),
        (locks, personas, sessions): Launchers,
        engine: app_agents::engine::EngineRoot,
    ) -> Option<AgentStack> {
        let kernel = self.broker.clone()?;
        let journal = self.undo.clone()?;
        let broker: Arc<dyn Broker> = kernel.broker.clone();
        let tickets = Arc::new(TicketLog::new(broker));
        // Computer use: narzędzia GUI przez ten sam rejestr kart Brokera (role z `gui.control`).
        let gate: Arc<dyn Broker> = tickets.clone();
        extra.extend(app_gui::gui_tools(
            gui,
            gate.clone(),
            Some(bus.clone()),
            monitor,
        ));
        let media = app_agents::MediaPorts::new(gui, sessions, locks.clone(), paths);
        let media = media.with(self.routers.as_ref(), self.audio.as_ref());
        let media = media.exec(self.exec.clone());
        // Zestaw multimediów trafia też do kill-switcha (zatrzymanie odtwarzania `media_play`).
        let (media_tools, media) =
            media.toolsets(gate.clone(), journal.clone(), platform_fs(options), bus);
        extra.extend(media_tools);
        if let Some(mcp) = &self.mcp {
            let mcp_gate = tools_common_contract::BrokerGate::new(gate.clone());
            extra.extend(app_mcp::agent_tools(mcp.clone(), mcp_gate));
        }
        if let Some(diag) = &self.diag {
            let gate = tools_common_contract::BrokerGate::new(gate.clone());
            extra.push(Arc::new(app_diag::DiagTool::new(diag.clone(), gate)));
        }
        let launch = app_agents::Launch::new(Some(locks), Some(gate), Some(personas));
        // Ta sama instancja, którą Broker zabija procesy (kill-switch, Job Objects).
        let exec = self.exec.clone()?;
        let clipboard: Arc<dyn platform_contract::ClipboardPort> = match &options.clipboard {
            Some(c) => c.clone(),
            None => Arc::new(platform_windows_impl::WindowsPlatform::default()),
        };
        let tools = AgentTools::new(ToolsDeps {
            broker: tickets.clone(),
            journal,
            fs: platform_fs(options),
            exec,
            clipboard: Some(clipboard),
            env: path_env_for(&paths.user_root).1,
            deny: app_modules::workdir::tool_deny_lists(paths),
            jobs: Some(kernel.jobs.clone()),
            bus: Some(bus.clone()),
            shell: ShellToolsConfig::detected(),
            base_env: None,
            extra,
            apps: Some(app_agents::AppsDeps::system(&paths.local)),
            media: Some(media),
        })
        .with_engine(app_agents::engine::engine_tools(
            engine,
            &paths.local,
            tickets.clone(),
            path_env_for(&paths.user_root).1,
            app_modules::workdir::tool_deny_lists(paths),
        ));
        Some(AgentStack {
            tools: Arc::new(tools),
            tickets,
            launch,
        })
    }

    /// Port głosu: urządzenia/test/czytanie z `voice-audio`+`voice-tts`, rozmowa — potok.
    pub(crate) fn voice_port(
        &self,
        options: &AppOptions,
        paths: &AppPaths,
        kernel: &super::Kernel,
        bus: &Arc<dyn EventBus>,
        scheduler: Arc<dyn scheduler_lite_contract::SchedulerLite>,
        gui: &app_gui::GuiPorts,
    ) -> Arc<dyn VoicePort> {
        let events = &kernel.events;
        if let Some(port) = &options.voice {
            return port.clone();
        }
        let noise = noise_setting(&kernel.config);
        let talk = super::voice_talk::talk_settings(&kernel.config);
        let base: Arc<dyn VoicePort> = match &self.audio {
            Some(audio) => Arc::new(
                app_modules::voice::VoiceAdapter::new(audio.io(), self.tts.clone(), events.clone())
                    .with_noise(noise.clone()),
            ),
            None => Arc::new(VoiceUnavailable),
        };
        let factory: Option<Arc<dyn VoiceEngineFactory>> =
            match (&options.voice_engine, &self.audio) {
                (Some(f), _) => Some(f.clone()),
                (None, Some(audio)) => Some(Arc::new(with_device(
                    SystemVoice::new(
                        paths.clone(),
                        audio.io(),
                        self.tts.clone(),
                        scheduler,
                        self.residency
                            .as_ref()
                            .map(|r| r.manager() as Arc<dyn model_residency_contract::Residency>),
                    )
                    .with_vault(Arc::new(StoreKeyVault::new(kernel.secrets.clone())))
                    .with_clock(Arc::new(DeviceClock))
                    .with_noise(noise)
                    .with_talk(talk)
                    .with_speech_meter(app_modules::speech_meter::SpeechMeter::new(events.clone())),
                    kernel.device.as_ref(),
                ))),
                (None, None) => None,
            };
        // F5: dyktowanie i czytanie przez porty computer use (strażnik okien Alfy/Brokera).
        let desktop = gui.available.then(|| DesktopDeps {
            desktop: gui.desktop.clone(),
            uia: gui.uia.clone(),
            input: gui.input.clone(),
            // Jak w narzędziach agentek: bez wstrzykniętego portu (powłoka go nie ustawia) schowek
            // systemowy. Wcześniej zostawało `None` i „Przeczytaj na głos" zawsze odmawiało
            // („unsupported"), niezależnie od zainstalowanych silników.
            clipboard: Some(match &options.clipboard {
                Some(c) => c.clone(),
                None => Arc::new(platform_windows_impl::WindowsPlatform::default())
                    as Arc<dyn platform_contract::ClipboardPort>,
            }),
        });
        let config = Some(kernel.config.clone() as Arc<dyn core_config_contract::ConfigStore>);
        let voice = PipelineVoice::new(
            base,
            factory,
            events.clone(),
            Some(bus.clone()),
            PipelineCfg::default(),
        );
        Arc::new(voice.with_features(FeatureDeps { config, desktop }))
    }
}

/// Zegar urządzenia audio: ta sama podstawa czasu, w której WASAPI znaczy ramki mikrofonu
/// i pozycję odtwarzania (QPC). Potok głosowy porównuje „teraz” ze znacznikami ramek.
struct DeviceClock;

impl voice_audio_contract::MediaClock for DeviceClock {
    fn now(&self) -> voice_audio_contract::MediaTime {
        voice_audio_contract::MediaTime(platform_windows_impl::qpc_nanos())
    }
}

/// Klucz ustawienia redukcji szumów mikrofonu (`off` / `standard` / `strong`).
const NOISE_KEY: &str = "voice.noise_reduction";

/// Ustawienie redukcji szumów śledzące konfigurację: test mikrofonu i rozmowa czytają je przy
/// każdym starcie, więc zmiana w Ustawieniach działa bez restartu programu.
fn noise_setting(
    config: &Arc<core_config_impl::FileConfigStore>,
) -> voice_dsp_contract::NoiseSetting {
    use core_config_contract::{ConfigKey, ConfigStore, Scope};
    use futures_util::StreamExt;

    let setting = voice_dsp_contract::NoiseSetting::default();
    let (config, shared) = (config.clone(), setting.clone());
    tokio::spawn(async move {
        let Ok(key) = ConfigKey::new(NOISE_KEY) else {
            return;
        };
        let mut changes = config.watch(NOISE_KEY);
        loop {
            let value = config.get(&key, &Scope::Global).await.ok().flatten();
            if let Some(text) = value.as_ref().and_then(|v| v.as_str()) {
                shared.set(voice_dsp_contract::NoiseReduction::parse(text));
            }
            if changes.next().await.is_none() {
                break;
            }
        }
    });
    setting
}

/// Serwery MCP użytkownika (`app-mcp`): lista i zgody w katalogu konfiguracji, wartości tajne
/// w magazynie sekretów Jądra, programy szukane w PATH (`npx` → `npx.cmd`).
pub(crate) fn mcp_hub(
    options: &AppOptions,
    paths: &AppPaths,
    kernel: &super::Kernel,
) -> Arc<app_mcp::McpHub> {
    let probe = options
        .cli_probe
        .clone()
        .unwrap_or_else(|| Arc::new(accounts_hub_impl::SystemCliProbe::from_env()));
    Arc::new(app_mcp::McpHub::new(app_mcp::McpHubDeps {
        config_dir: paths.config.clone(),
        work_dir: paths.local.join("mcp"),
        secrets: Some(kernel.secrets.clone()),
        probe: Some(probe),
        connector: Arc::new(app_mcp::StdioConnector),
    }))
}

/// Diagnostyka komputera: historia w danych lokalnych, raporty w `<Alfa>\Diagnostyka`,
/// codzienny zapis parametrów w tle.
pub(crate) async fn diag_app(
    options: &AppOptions,
    paths: &AppPaths,
    kernel: &super::Kernel,
) -> Arc<app_diag::DiagApp> {
    let probe = options
        .diag_probe
        .clone()
        .unwrap_or_else(app_diag::system_probe);
    let app = Arc::new(app_diag::DiagApp::new(app_diag::DiagAppDeps {
        probe,
        local: paths.local.clone(),
        reports: paths.user_root.join("Diagnostyka"),
    }));
    // Flaga eksperymentalna (Ustawienia → Zaawansowane), domyślnie włączona.
    let daily = super::memory::setting(kernel, "experimental.diag_daily").await;
    if daily.and_then(|v| v.as_bool()).unwrap_or(true) {
        app.spawn_auto();
    }
    app
}

/// Profil sprzętu dla głosu: dzierżawa Whispera wskazuje kartę z profilu (laptop z dwiema
/// kartami); bez profilu — karta główna, jak wcześniej.
fn with_device(
    voice: SystemVoice,
    device: Option<&Arc<dyn device_profile_contract::DeviceProfile>>,
) -> SystemVoice {
    match device {
        Some(d) => voice.with_device(d.clone()),
        None => voice,
    }
}
