//! Części kompozycji: jądro (sekrety, katalog, konfiguracja, logi, zdarzenia, sprzęt) i budowa
//! poszczególnych modułów wywoływana w kolejności z rejestru (`compose.rs`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use accounts_hub_contract::SecretStore;
use accounts_hub_impl::{AccountsHubService, JsonFileRepository};
use artifacts_impl::SqliteArtifacts;
use compliance_impl::ComplianceService;
use core_bus_contract::EventBus;
use core_config_contract::MachineId;
use core_config_impl::{ConfigOptions, FileConfigStore};
use core_registry_contract::HealthStatus;
use core_registry_impl::ModuleRegistry;
use cost_meter_contract::{BudgetConfig, FxSource, LimitMode, MonthlyLimit};
use cost_meter_impl::{CostMeterService, NbpFxSource, NdjsonLedger};
use device_profile_contract::{DeviceProfile as DeviceProfileService, MachineOverlay};
use device_profile_impl::DeviceProfileConfig;
use memory_impl::SqliteMemory;
use personas_impl::PersonasModule;
use providers_api_impl::ProvidersApiModule;
use scheduler_lite_impl::SchedulerModule;
use search_contract::TxIndexer;
use search_impl::SqliteSearch;
use sessions_contract::SessionDbProvider;
use sessions_impl::{SessionsConfig, SqliteSessions};

use crate::brain::DirectBrain;
use crate::compose::{HealthSlot, internal, started};
use crate::core::{AppCore, Inner, Runtime};
use crate::error::AppError;
use crate::events::EventHub;
use crate::infra::catalog::ProviderCatalog;
use crate::infra::embedder::LexicalEmbedder;
use crate::infra::http::{OfflineFx, ReqwestGet};
use crate::infra::late::{LateDbProvider, LateIndexer};
use crate::infra::probe::ProviderProbe;
use crate::infra::secrets::{StoreKeyVault, system_secret_store};
use crate::options::{AppOptions, AppPaths};
use crate::ports::{BrokerUnavailable, HeadlessShell, TransferUnavailable, VoiceUnavailable};
use crate::settings::{SettingsCatalog, keys};
use crate::store::AppStore;

/// Jądro: wszystko, co nie jest modułem rejestru albo jest potrzebne przed modułami.
pub(crate) struct Kernel {
    pub secrets: Arc<dyn SecretStore>,
    pub catalog: ProviderCatalog,
    pub config: Arc<FileConfigStore>,
    pub machine: MachineId,
    pub events: EventHub,
    pub settings: SettingsCatalog,
    pub device_pending: Option<device_profile_impl::DeviceProfileService>,
    pub device: Option<Arc<dyn DeviceProfileService>>,
}

fn detect_device(paths: &AppPaths) -> Result<device_profile_impl::DeviceProfileService, AppError> {
    let config = DeviceProfileConfig {
        state_dir: paths.state(),
        overlay: MachineOverlay::default(),
    };
    #[cfg(windows)]
    let detected = device_profile_impl::DeviceProfileService::detect(
        Arc::new(platform_windows_impl::WinHardware),
        None,
        config,
    );
    #[cfg(not(windows))]
    let detected = device_profile_impl::DeviceProfileService::detect_native(config);
    detected.map_err(|e| internal("profil urządzenia")(e.to_string()))
}

impl Kernel {
    /// Sekrety, katalog, sprzęt (identyfikator maszyny), konfiguracja, logi, zdarzenia.
    pub async fn start(
        paths: &AppPaths,
        options: &AppOptions,
        bus: &Arc<dyn EventBus>,
    ) -> Result<Self, AppError> {
        let secrets = match &options.secrets {
            Some(s) => s.clone(),
            None => system_secret_store()
                .map_err(|e| AppError::new(crate::error::ErrorCode::Secrets, e.to_string()))?,
        };
        let (device_pending, device, machine_id) = match &options.device {
            Some(d) => (
                None,
                Some(d.clone()),
                d.current().machine_id.as_str().to_owned(),
            ),
            None => {
                let service = detect_device(paths)?;
                let id = service.current().machine_id.as_str().to_owned();
                (Some(service), None, id)
            }
        };
        let machine = MachineId::new(machine_id);
        let clock: Arc<dyn core_config_impl::Clock> = Arc::new(chrono::Utc::now);
        let config =
            FileConfigStore::open(ConfigOptions::new(&paths.config, machine.clone()), clock)
                .map_err(AppError::from)?
                .with_bus(bus.clone());
        for problem in config.load_problems() {
            tracing::warn!(problem, "plik konfiguracji pominięty");
        }
        let settings = SettingsCatalog::builtin()?;
        for (prefix, schema) in settings.schemas()? {
            config
                .register_schema(&prefix, &schema)
                .map_err(AppError::from)?;
        }
        if options.file_logs {
            start_file_logs(paths, bus).await;
        }
        Ok(Self {
            secrets,
            catalog: ProviderCatalog::builtin(),
            config: Arc::new(config),
            machine,
            events: EventHub::start(options.frame),
            settings,
            device_pending,
            device,
        })
    }
}

async fn start_file_logs(paths: &AppPaths, bus: &Arc<dyn EventBus>) {
    let sink = match core_log_impl::FileLogSink::open(core_log_impl::LogOptions::new(paths.logs()))
    {
        Ok(sink) => Arc::new(sink),
        Err(e) => {
            tracing::error!(error = %e, "logi NDJSON niedostępne");
            return;
        }
    };
    if let Err(e) = core_log_impl::spawn_bus_writer(bus.clone(), sink, None).await {
        tracing::error!(error = %e, "zapis logów z magistrali nie wystartował");
    }
}

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
                let service = cost_meter(paths, options, kernel).await?;
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
            other => tracing::warn!(modul = other, "moduł bez reguły budowy — pominięty"),
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
        let brain = match options.brain {
            Some(b) => b,
            None => Arc::new(DirectBrain::new(
                hub.clone(),
                Arc::new(kernel.catalog.api.clone()),
            )),
        };
        let provider: Arc<dyn SessionDbProvider> = sessions.clone();
        let inner = Inner {
            undo_window: options.undo_window,
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
            _scheduler: need(&self.scheduler, "scheduler-lite")?,
            hub,
            sessions,
            device,
            brain,
            transfer: options
                .transfer
                .unwrap_or_else(|| Arc::new(TransferUnavailable)),
            voice: options.voice.unwrap_or_else(|| Arc::new(VoiceUnavailable)),
            broker: options
                .broker
                .unwrap_or_else(|| Arc::new(BrokerUnavailable)),
            shell: options
                .shell
                .unwrap_or_else(|| Arc::new(HeadlessShell::default())),
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

async fn cost_meter(
    paths: &AppPaths,
    options: &AppOptions,
    kernel: &Kernel,
) -> Result<CostMeterService, AppError> {
    use core_config_contract::{ConfigKey, ConfigStore, Scope};
    let get = |key: &str| {
        let config = kernel.config.clone();
        let key = ConfigKey::new(key);
        async move {
            match key {
                Ok(k) => config.get(&k, &Scope::Global).await.ok().flatten(),
                Err(_) => None,
            }
        }
    };
    let enabled = get(keys::COST_LIMIT_ENABLED)
        .await
        .and_then(|v| v.as_bool());
    let grosze = get(keys::COST_LIMIT_GROSZE).await.and_then(|v| v.as_u64());
    let mut budget = BudgetConfig::default();
    if let Some(grosze) = grosze {
        let mode = if enabled.unwrap_or(true) {
            LimitMode::Enforced
        } else {
            LimitMode::AlertOnly
        };
        budget.monthly = MonthlyLimit {
            amount_micro_pln: cost_meter_contract::grosze_to_micro_pln(grosze),
            mode,
        };
    }
    let fx: Arc<dyn FxSource> = match (options.fetch_fx, ReqwestGet::new()) {
        (true, Some(http)) => Arc::new(NbpFxSource::new(http)),
        _ => Arc::new(OfflineFx),
    };
    CostMeterService::new(
        Arc::new(NdjsonLedger::new(paths.state().join("costs.ndjson"))),
        fx,
        Arc::new(cost_meter_impl::SystemClock),
        budget,
    )
    .map_err(AppError::from)
}
