//! Narzędzia agentek (`tools-fs/shell/clipboard` nad Brokerem w procesie i dziennikiem cofania)
//! i port głosu z potokiem rozmowy — składane po zbudowaniu modułów (jedno miejsce kompozycji).

use std::sync::Arc;

use app_agents::{AgentTools, ShellToolsConfig, TicketLog, ToolsDeps};
use app_modules::broker::path_env_for;
use app_voice::{PipelineVoice, SystemVoice, VoiceEngineFactory};
use compliance_contract::DenyLists;
use core_bus_contract::EventBus;
use safety_broker_contract::Broker;
use voice_pipeline_contract::PipelineCfg;

use super::Extra;
use super::extra::platform_fs;
use crate::options::{AppOptions, AppPaths};
use crate::ports::{VoicePort, VoiceUnavailable};

/// Narzędzia agentek i rejestr próśb o zatwierdzenie.
#[derive(Clone)]
pub(crate) struct AgentStack {
    /// Narzędzia (wszystkie przebiegi).
    pub tools: Arc<AgentTools>,
    /// Fakty próśb o zatwierdzenie (karta w wątku).
    pub tickets: Arc<TicketLog>,
}

impl Extra {
    /// Narzędzia agentek — tylko z Brokerem i dziennikiem cofania (bez nich agentki odpowiadają
    /// bez narzędzi; żadna akcja nie omija Brokera).
    pub(crate) fn agent_stack(
        &self,
        options: &AppOptions,
        paths: &AppPaths,
        bus: &Arc<dyn EventBus>,
    ) -> Option<AgentStack> {
        let engine = self.broker.clone()?;
        let journal = self.undo.clone()?;
        let broker: Arc<dyn Broker> = engine.clone();
        let tickets = Arc::new(TicketLog::new(broker));
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
            deny: DenyLists::baseline(),
            jobs: Some(engine),
            bus: Some(bus.clone()),
            shell: ShellToolsConfig::default(),
            base_env: None,
        });
        Some(AgentStack {
            tools: Arc::new(tools),
            tickets,
        })
    }

    /// Port głosu: urządzenia/test/czytanie z `voice-audio`+`voice-tts`, rozmowa — potok.
    pub(crate) fn voice_port(
        &self,
        options: &AppOptions,
        paths: &AppPaths,
        events: &crate::events::EventHub,
        bus: &Arc<dyn EventBus>,
        scheduler: Arc<dyn scheduler_lite_contract::SchedulerLite>,
    ) -> Arc<dyn VoicePort> {
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
                (None, Some(audio)) => Some(Arc::new(SystemVoice::new(
                    paths.clone(),
                    audio.io(),
                    self.tts.clone(),
                    scheduler,
                    self.residency
                        .as_ref()
                        .map(|r| r.manager() as Arc<dyn model_residency_contract::Residency>),
                ))),
                (None, None) => None,
            };
        Arc::new(PipelineVoice::new(
            base,
            factory,
            events.clone(),
            Some(bus.clone()),
            PipelineCfg::default(),
        ))
    }
}
