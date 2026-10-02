//! Części kompozycji: jądro (sekrety, katalog, konfiguracja, logi, zdarzenia, sprzęt) i budowa
//! poszczególnych modułów wywoływana w kolejności z rejestru (`compose.rs`).

mod agents;
mod extra;
mod kernel;
mod memory;
mod ports;
pub(crate) mod signals;
mod tasks;
mod work;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use accounts_hub_impl::{AccountsHubService, JsonFileRepository};
use artifacts_impl::SqliteArtifacts;
use compliance_impl::ComplianceService;
use core_bus_contract::EventBus;
use core_registry_impl::ModuleRegistry;
use cost_meter_impl::CostMeterService;
use memory_consolidation_impl::ConsolidationModule;
use memory_impl::MemoryModule;
use personas_impl::PersonasModule;
use providers_api_impl::ProvidersApiModule;
use search_contract::TxIndexer;
use search_impl::SqliteSearch;
use sessions_contract::SessionDbProvider;
use sessions_impl::{SessionsConfig, SqliteSessions};

use crate::compose::{HealthSlot, internal, started};
use crate::core::{AppCore, Inner, Runtime};
use crate::error::AppError;
use crate::infra::probe::ProviderProbe;
use crate::options::{AppOptions, AppPaths};
use crate::ports::HeadlessShell;
pub(crate) use agents::AgentStack;
use app_modules::embedder::LexicalEmbedder;
use app_modules::late::{LateDbProvider, LateIndexer};
use app_modules::secrets::StoreKeyVault;
use app_store::AppStore;
pub(crate) use extra::Extra;
pub(crate) use kernel::Kernel;
pub(crate) use work::WorkStack;

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
    memory: Option<Arc<MemoryModule>>,
    privacy: Option<Arc<dyn memory_contract::PrivacyOracle>>,
    consolidation: Option<Arc<ConsolidationModule>>,
    artifacts: Option<Arc<SqliteArtifacts>>,
    personas: Option<Arc<PersonasModule>>,
    tasks: tasks::TaskParts,
    extra: Extra,
    /// Gniazda zdrowia modułów `app-gui/terminal/skills/health` (wypełniane po złożeniu).
    work_slots: Vec<(String, HealthSlot)>,
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
        let healthy = || extra::healthy(slot);
        if work::is_work(id) {
            self.work_slots
                .extend(slot.map(|s| (id.to_owned(), s.clone())));
            return Ok(());
        }
        if self.build_tasks(id, paths, kernel, bus, slot).await? {
            return Ok(());
        }
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
            "memory" => self.build_memory(paths, kernel, bus, slot).await?,
            "memory-consolidation" => {
                if let Err(e) = self.build_consolidation(kernel, bus, slot).await {
                    extra::unhealthy(slot, id, &e);
                }
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
                    memory: self
                        .memory
                        .as_ref()
                        .map(|m| m.service() as Arc<dyn memory_contract::MemoryService>)
                        .zip(self.privacy.clone()),
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
    pub async fn into_core(
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
        let scheduler = need(&self.tasks.scheduler, "scheduler")?;
        let memory = self.memory_app(&kernel).await?;
        let personas = need(&self.personas, "personas")?;
        let (gui, monitor) = work::gui(&options, &paths, &kernel);
        let ports = self.extra.ports(
            &options,
            ports::PortDeps {
                hub: &hub,
                kernel: &kernel,
                sessions: &sessions,
                shell: &shell,
                paths: &paths,
                bus: &bus,
                scheduler,
                tools: memory.tools(),
                gui: (&gui, &monitor),
                personas: personas.clone(),
            },
        )?;
        let stack = self.task_stack(tasks::StackDeps {
            options: &options,
            paths: &paths,
            kernel: &kernel,
            bus: &bus,
            shell: &shell,
            brain: &ports.brain,
            agents: ports.agents.as_ref(),
            translator: options.brain.is_some() || self.extra.routers.is_some(),
        })?;
        let work = self
            .work_stack(work::WorkDepsIn {
                options: &options,
                paths: &paths,
                kernel: &kernel,
                registry: registry.clone(),
                bus: &bus,
                monitor,
                broker: ports.broker.clone(),
                shell: &shell,
                voice: ports.voice.clone(),
                tasks: stack.tasks.clone(),
                personas: personas.clone(),
                stack: ports.agents.as_ref(),
            })
            .await;
        work.health.bind_brain(ports.brain.clone());
        let mut extra = self.extra;
        let ready = match ports.agents {
            Some(_) => Ok(()),
            None => Err("brak Brokera albo dziennika cofania".to_owned()),
        };
        let _ = extra.agents_ready.set(ready);
        let keep = [self
            .providers
            .map(|p| p as Arc<dyn std::any::Any + Send + Sync>)];
        extra.keep.extend(keep.into_iter().flatten());
        extra.keep.extend(
            self.memory
                .map(|m| m as Arc<dyn std::any::Any + Send + Sync>),
        );
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
            memory,
            artifacts: need(&self.artifacts, "artifacts")?,
            costs: need(&self.costs, "cost-meter")?,
            _compliance: need(&self.compliance, "compliance")?,
            personas,
            work,
            signals: kernel.signals,
            tasks: stack.tasks,
            bridges: stack.bridges,
            hub,
            sessions,
            device,
            brain: ports.brain,
            extra,
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
        let core = AppCore {
            inner: Arc::new(inner),
        };
        stack.binder.bind(&core);
        Ok(core)
    }
}
