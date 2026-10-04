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

/// Zasoby wyłączne i obsada dla startu przebiegów v1.
pub(crate) type Launchers = (
    Arc<dyn scheduler_lite_contract::SchedulerLite>,
    Arc<dyn personas_contract::Personas>,
);

/// Narzędzia agentek i rejestr próśb o zatwierdzenie.
#[derive(Clone)]
pub(crate) struct AgentStack {
    /// Narzędzia (wszystkie przebiegi).
    pub tools: Arc<AgentTools>,
    /// Fakty próśb o zatwierdzenie (karta w wątku).
    pub tickets: Arc<TicketLog>,
    /// Start v1: zasoby wyłączne (scheduler), autonomia (Broker), obsada, umiejętności.
    pub launch: app_agents::Launch,
}

impl Extra {
    /// Narzędzia agentek — tylko z Brokerem i dziennikiem cofania (bez nich agentki odpowiadają
    /// bez narzędzi; żadna akcja nie omija Brokera).
    pub(crate) fn agent_stack(
        &self,
        options: &AppOptions,
        paths: &AppPaths,
        bus: &Arc<dyn EventBus>,
        mut extra: Vec<Arc<dyn tools_common_contract::Tool>>,
        (gui, monitor): (&app_gui::GuiPorts, &Arc<app_gui::GuiMonitor>),
        (locks, personas): Launchers,
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
            shell: ShellToolsConfig::default(),
            base_env: None,
            extra,
            apps: Some(app_agents::AppsDeps::system(&paths.local)),
        });
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
        let base: Arc<dyn VoicePort> = match &self.audio {
            Some(audio) => Arc::new(app_modules::voice::VoiceAdapter::new(
                audio.io(),
                self.tts.clone(),
                events.clone(),
            )),
            None => Arc::new(VoiceUnavailable),
        };
        let factory: Option<Arc<dyn VoiceEngineFactory>> =
            match (&options.voice_engine, &self.audio) {
                (Some(f), _) => Some(f.clone()),
                (None, Some(audio)) => Some(Arc::new(
                    SystemVoice::new(
                        paths.clone(),
                        audio.io(),
                        self.tts.clone(),
                        scheduler,
                        self.residency
                            .as_ref()
                            .map(|r| r.manager() as Arc<dyn model_residency_contract::Residency>),
                    )
                    .with_vault(Arc::new(StoreKeyVault::new(kernel.secrets.clone()))),
                )),
                (None, None) => None,
            };
        // F5: dyktowanie i czytanie przez porty computer use (strażnik okien Alfy/Brokera).
        let desktop = gui.available.then(|| DesktopDeps {
            desktop: gui.desktop.clone(),
            uia: gui.uia.clone(),
            input: gui.input.clone(),
            clipboard: options.clipboard.clone(),
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
