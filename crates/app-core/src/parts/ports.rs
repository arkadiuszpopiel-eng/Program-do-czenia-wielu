//! Porty `AppCore` z modułów podpiętych po F1 (jedno miejsce kompozycji): nadpisanie z
//! `AppOptions` → adapter na module → „moduł niepodłączony".

use std::sync::Arc;

use accounts_hub_impl::AccountsHubService;
use app_modules::broker::{InprocBroker, path_env};
use app_modules::transfer::TransferAdapter;
use app_modules::voice::VoiceAdapter;
use sessions_contract::SessionCatalog;
use sessions_impl::SqliteSessions;

use super::{Extra, Kernel};
use crate::error::AppError;
use crate::options::AppOptions;
use crate::ports::{
    BrainPort, BrokerPort, BrokerUnavailable, NoApprovalWindow, ShellPort, TransferPort,
    TransferUnavailable, VoicePort, VoiceUnavailable,
};
use crate::route::{RouterBrain, RouterUnavailable};

/// Porty złożone z modułów.
pub(crate) struct Ports {
    pub brain: Arc<dyn BrainPort>,
    pub transfer: Arc<dyn TransferPort>,
    pub voice: Arc<dyn VoicePort>,
    pub broker: Arc<dyn BrokerPort>,
}

impl Extra {
    /// Porty: nadpisanie z opcji → adapter modułu → moduł niepodłączony.
    pub fn ports(
        &self,
        options: &AppOptions,
        hub: &Arc<AccountsHubService>,
        kernel: &Kernel,
        sessions: &Arc<SqliteSessions>,
        shell: &Arc<dyn ShellPort>,
    ) -> Result<Ports, AppError> {
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
        let voice: Arc<dyn VoicePort> = match (&options.voice, &self.audio) {
            (Some(port), _) => port.clone(),
            (None, Some(audio)) => Arc::new(VoiceAdapter::new(
                audio.io(),
                self.tts.clone(),
                kernel.events.clone(),
            )),
            (None, None) => Arc::new(VoiceUnavailable),
        };
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
                    path_env().1,
                ))
            }
            (None, None) => Arc::new(BrokerUnavailable),
        };
        Ok(Ports {
            brain,
            transfer,
            voice,
            broker,
        })
    }
}
