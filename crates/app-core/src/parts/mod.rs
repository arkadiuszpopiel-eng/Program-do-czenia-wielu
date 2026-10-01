//! Części kompozycji: jądro (sekrety, katalog, konfiguracja, logi, zdarzenia, sprzęt) i budowa
//! poszczególnych modułów wywoływana w kolejności z rejestru (`compose.rs`).

mod agents;
mod extra;
mod kernel;
mod ports;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use accounts_hub_impl::{AccountsHubService, JsonFileRepository};
use artifacts_impl::SqliteArtifacts;
use compliance_impl::ComplianceService;
use core_bus_contract::EventBus;
use core_registry_contract::HealthStatus;
use core_registry_impl::ModuleRegistry;
use cost_meter_impl::CostMeterService;
use memory_impl::SqliteMemory;
use personas_impl::PersonasModule;
use providers_api_impl::ProvidersApiModule;
use scheduler_lite_impl::SchedulerModule;
use search_contract::TxIndexer;
use search_impl::SqliteSearch;
use sessions_contract::SessionDbProvider;
use sessions_impl::{SessionsConfig, SqliteSessions};

use crate::compose::{HealthSlot, internal, started};
use crate::core::{AppCore, Inner, Runtime};
use crate::error::AppError;
use crate::infra::embedder::LexicalEmbedder;
use crate::infra::late::{LateDbProvider, LateIndexer};
use crate::infra::probe::ProviderProbe;
use crate::infra::secrets::StoreKeyVault;
use crate::options::{AppOptions, AppPaths};
use crate::ports::HeadlessShell;
use crate::store::AppStore;
pub(crate) use agents::AgentStack;
pub(crate) use extra::Extra;
pub(crate) use kernel::Kernel;

/// Moduły zbudowane w kolejności rejestru.
#[derive(Default)]
pub(crate) struct Built {
    compliance: Option<Arc<ComplianceService>>,
    hub: Option<Arc<AccountsHubService>>,
    providers: Option<Arc<ProvidersApiModule>>,
    costs: Option<Arc<CostMeterService>>,
    sessions: Option<Arc<SqliteSessions>>,
    late_db: Arc<LateDbProvider>,
    late_index: Arc<LateIndexer>,
    search: Option<Arc<SqliteSearch>>,
    memory: Option<Arc<SqliteMemory>>,
    artifacts: Option<Arc<SqliteArtifacts>>,
    personas: Option<Arc<PersonasModule>>,
    scheduler: Option<Arc<SchedulerModule>>,
    extra: Extra,
}

fn need<T: ?Sized>(value: &Option<Arc<T>>, what: &str) -> Result<Arc<T>, AppError> {
    value
        .clone()
        .ok_or_else(|| AppError::internal(format!("moduł {what} nie został zbudowany")))
}

impl Built {
    /// Buduje i uruchamia moduł `id` (zależności są już zbudowane — kolejność z rejestru).
    pub async fn build_module(
        &mut self,
        id: &str,
        paths: &AppPaths,
        options: &AppOptions,
        bus: &Arc<dyn EventBus>,
        kernel: &mut Kernel,
        slot: Option<&HealthSlot>,
    ) -> Result<(), AppError> {
        let healthy = || {
            if let Some(slot) = slot {
                let _ = slot.set(Arc::new(|| HealthStatus::Healthy));
            }
        };
        match id {
            "platform-windows" => healthy(),
            "device-profile" => match kernel.device_pending.take() {
                Some(service) => {
                    let service = started(service, bus, slot).await?;
                    kernel.device = Some(service);
                }
                None => healthy(),
            },
            "compliance" => {
                let policy = kernel
                    .catalog
                    .hub
                    .iter()
                    .map(|e| e.policy_input())
                    .collect();
                let service = ComplianceService::with_default_registry(policy)
                    .map_err(|e| internal("compliance")(e.to_string()))?;
                self.compliance = Some(started(service, bus, slot).await?);
            }
            "accounts-hub" => {
                let probe = Arc::new(ProviderProbe::new(Arc::new(kernel.catalog.api.clone())));
                let compliance: Arc<dyn compliance_contract::Compliance> =
                    need(&self.compliance, "compliance")?;
                let hub = AccountsHubService::builder(
                    kernel.catalog.hub.clone(),
                    kernel.secrets.clone(),
                    probe.clone(),
                    probe,
                )
                .repository(Arc::new(JsonFileRepository::new(
                    paths.config.join("accounts.json"),
                )))
                .compliance(compliance)
                .build()
                .map_err(AppError::from)?;
                self.hub = Some(started(hub, bus, slot).await?);
            }
            "providers-api" => {
                let module = ProvidersApiModule::new()
                    .map_err(|e| internal("providers-api")(e.to_string()))?;
                self.providers = Some(started(module, bus, slot).await?);
            }
            "cost-meter" => {
                let service = kernel::cost_meter(paths, options, kernel).await?;
                self.costs = Some(started(service, bus, slot).await?);
            }
            "sessions" => {
                let vault = Arc::new(StoreKeyVault::new(kernel.secrets.clone()));
                let config = SessionsConfig {
                    data_dir: paths.sessions(),
                    workdir_root: paths.workdirs(),
                };
                let sessions = SqliteSessions::open(config, vault)
                    .map_err(AppError::from)?
                    .with_indexer(self.late_index.clone());
                let sessions = started(sessions, bus, slot).await?;
                let provider: Arc<dyn SessionDbProvider> = sessions.clone();
                self.late_db.bind(&provider);
                self.sessions = Some(sessions);
            }
            "search" => {
                let search = SqliteSearch::new(self.late_db.clone(), Arc::new(LexicalEmbedder))
                    .map_err(|e| internal("search")(e.to_string()))?;
                let search = started(search, bus, slot).await?;
                let indexer: Arc<dyn TxIndexer> = search.clone();
                self.late_index.bind(&indexer);
                self.search = Some(search);
            }
            "memory" => {
                let search = need(&self.search, "search")?;
                let memory = SqliteMemory::new(self.late_db.clone(), search.clone(), search)
                    .map_err(|e| internal("memory")(e.to_string()))?;
                self.memory = Some(started(memory, bus, slot).await?);
            }
            "artifacts" => {
                let artifacts = SqliteArtifacts::new(self.late_db.clone(), paths.user_root.clone())
                    .map_err(|e| internal("artifacts")(e.to_string()))?;
                self.artifacts = Some(started(artifacts, bus, slot).await?);
            }
            "personas" => {
                let personas =
                    PersonasModule::new().map_err(|e| internal("personas")(e.to_string()))?;
                self.personas = Some(started(personas, bus, slot).await?);
            }
            "scheduler-lite" => {
                let scheduler = SchedulerModule::new()
                    .map_err(|e| internal("scheduler-lite")(e.to_string()))?;
                self.scheduler = Some(started(scheduler, bus, slot).await?);
            }
            other => {
                let deps = extra::Deps {
                    paths,
                    options,
                    bus,
                    kernel: &*kernel,
                    compliance: self
                        .compliance
                        .clone()
                        .map(|c| c as Arc<dyn compliance_contract::Compliance>),
                    costs: self.costs.clone(),
                    sessions: self.sessions.clone(),
                    slot,
                };
                if !self.extra.build(other, deps).await {
                    tracing::warn!(modul = other, "moduł bez reguły budowy — pominięty");
                }
            }
        }
        Ok(())
    }

    /// Składa `AppCore` z gotowych modułów i portów.
    pub fn into_core(
        self,
        paths: AppPaths,
        options: AppOptions,
        bus: Arc<dyn EventBus>,
        registry: Arc<ModuleRegistry>,
        kernel: Kernel,
    ) -> Result<AppCore, AppError> {
        let hub = need(&self.hub, "accounts-hub")?;
        let sessions = need(&self.sessions, "sessions")?;
        let device = kernel
            .device
            .clone()
            .ok_or_else(|| AppError::internal("brak profilu urządzenia"))?;
        let shell = options
            .shell
            .clone()
            .unwrap_or_else(|| Arc::new(HeadlessShell::default()));
        let scheduler = need(&self.scheduler, "scheduler-lite")?;
        let ports = self.extra.ports(
            &options,
            ports::PortDeps {
                hub: &hub,
                kernel: &kernel,
                sessions: &sessions,
                shell: &shell,
                paths: &paths,
                bus: &bus,
                scheduler: scheduler.clone(),
            },
        )?;
        let provider: Arc<dyn SessionDbProvider> = sessions.clone();
        let inner = Inner {
            undo_window: options.undo_window,
            approval_timeout: options.approval_timeout,
            healthy_after: options.healthy_after,
            app_version: options.app_version,
            bus,
            registry,
            config: kernel.config,
            machine: kernel.machine,
            search: need(&self.search, "search")?,
            memory: need(&self.memory, "memory")?,
            artifacts: need(&self.artifacts, "artifacts")?,
            costs: need(&self.costs, "cost-meter")?,
            _compliance: need(&self.compliance, "compliance")?,
            personas: need(&self.personas, "personas")?,
            _scheduler: scheduler,
            hub,
            sessions,
            device,
            brain: ports.brain,
            extra: self.extra,
            transfer: ports.transfer,
            voice: ports.voice,
            broker: ports.broker,
            agents: ports.agents,
            shell,
            events: kernel.events,
            store: AppStore::new(provider),
            settings: kernel.settings,
            runtime: Mutex::new(Runtime {
                online: true,
                ..Runtime::default()
            }),
            locks: Mutex::new(HashMap::new()),
            paths,
        };
        Ok(AppCore {
            inner: Arc::new(inner),
        })
    }
}
