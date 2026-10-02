//! Porty `AppCore` z modułów podpiętych po F1 (jedno miejsce kompozycji): nadpisanie z
//! `AppOptions` → adapter na module → „moduł niepodłączony".

use std::sync::Arc;

use accounts_hub_impl::AccountsHubService;
use app_modules::broker::{InprocBroker, path_env_for};
use app_modules::transfer::TransferAdapter;
use sessions_contract::SessionCatalog;
use sessions_impl::SqliteSessions;

use super::agents::AgentStack;
use super::{Extra, Kernel};
use crate::error::AppError;
use crate::options::AppOptions;
use crate::ports::{
    BrainPort, BrokerPort, BrokerUnavailable, NoApprovalWindow, ShellPort, TransferPort,
    TransferUnavailable, VoicePort,
};
use app_modules::route::{RouterBrain, RouterUnavailable};

/// Porty złożone z modułów.
pub(crate) struct Ports {
    pub brain: Arc<dyn BrainPort>,
    pub transfer: Arc<dyn TransferPort>,
    pub voice: Arc<dyn VoicePort>,
    pub broker: Arc<dyn BrokerPort>,
    pub agents: Option<AgentStack>,
}

/// Wspólne zależności portów.
pub(crate) struct PortDeps<'a> {
    pub hub: &'a Arc<AccountsHubService>,
    pub kernel: &'a Kernel,
    pub sessions: &'a Arc<SqliteSessions>,
    pub shell: &'a Arc<dyn ShellPort>,
    pub paths: &'a crate::options::AppPaths,
    pub bus: &'a Arc<dyn core_bus_contract::EventBus>,
    pub scheduler: Arc<dyn scheduler_lite_contract::SchedulerLite>,
    /// Narzędzia spoza `tools-*` (pamięć) dla agentek.
    pub tools: Vec<Arc<dyn tools_common_contract::Tool>>,
    /// Porty GUI i panel „Ekran" (narzędzia computer use dla ról z `gui.control`).
    pub gui: (&'a app_gui::GuiPorts, &'a Arc<app_gui::GuiMonitor>),
    /// Persony (obsada przebiegów: delegacja, Krytyczka).
    pub personas: Arc<personas_impl::PersonasModule>,
}

impl Extra {
    /// Porty: nadpisanie z opcji → adapter modułu → moduł niepodłączony.
    pub fn ports(&self, options: &AppOptions, deps: PortDeps<'_>) -> Result<Ports, AppError> {
        let PortDeps {
            hub,
            kernel,
            sessions,
            shell,
            paths,
            bus,
            scheduler,
            tools,
            gui,
            personas,
        } = deps;
        let brain: Arc<dyn BrainPort> = match (&options.brain, &self.routers) {
            (Some(brain), _) => brain.clone(),
            (None, Some(routers)) => Arc::new(RouterBrain::new(
                routers.clone(),
                hub.clone(),
                Arc::new(kernel.catalog.api.clone()),
            )),
            (None, None) => Arc::new(RouterUnavailable),
        };
        let transfer: Arc<dyn TransferPort> = match (&options.transfer, &self.transfer) {
            (Some(port), _) => port.clone(),
            (None, Some(module)) => {
                let catalog: Arc<dyn SessionCatalog> = sessions.clone();
                Arc::new(TransferAdapter::new(
                    module.clone(),
                    shell.clone(),
                    catalog,
                    kernel.config.clone(),
                ))
            }
            (None, None) => Arc::new(TransferUnavailable),
        };
        let launch = (
            scheduler.clone(),
            personas as Arc<dyn personas_contract::Personas>,
        );
        let voice: Arc<dyn VoicePort> =
            self.voice_port(options, paths, &kernel.events, bus, scheduler);
        let broker: Arc<dyn BrokerPort> = match (&options.broker, &self.broker) {
            (Some(port), _) => port.clone(),
            (None, Some(engine)) => {
                let window = options
                    .approval_window
                    .clone()
                    .unwrap_or_else(|| Arc::new(NoApprovalWindow));
                Arc::new(InprocBroker::new(
                    engine.clone(),
                    self.undo.clone(),
                    window,
                    path_env_for(&paths.user_root).1,
                ))
            }
            (None, None) => Arc::new(BrokerUnavailable),
        };
        Ok(Ports {
            brain,
            transfer,
            voice,
            broker,
            agents: self.agent_stack(options, paths, bus, tools, gui, launch),
        })
    }
}
