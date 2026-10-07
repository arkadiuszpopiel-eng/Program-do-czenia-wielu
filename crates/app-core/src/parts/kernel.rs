//! Jądro kompozycji: sekrety, katalog dostawców, sprzęt (`MachineId`), konfiguracja, logi NDJSON,
//! zdarzenia UI — oraz licznik kosztów (zależny od konfiguracji limitu).

use std::sync::Arc;

use accounts_hub_contract::SecretStore;
use core_bus_contract::EventBus;
use core_config_contract::MachineId;
use core_config_impl::{ConfigOptions, FileConfigStore};
use cost_meter_contract::{BudgetConfig, FxSource, LimitMode, MonthlyLimit};
use cost_meter_impl::{CostMeterService, NbpFxSource, NdjsonLedger};
use device_profile_contract::{DeviceProfile as DeviceProfileService, MachineOverlay};
use device_profile_impl::DeviceProfileConfig;

use crate::compose::internal;
use crate::error::AppError;
use crate::events::EventHub;
use crate::infra::catalog::ProviderCatalog;
use crate::infra::http::{OfflineFx, ReqwestGet};
use crate::options::{AppOptions, AppPaths};
use crate::settings::{SettingsCatalog, keys};
use app_modules::secrets::system_secret_store;

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
    /// Sygnały systemowe (`parts/signals.rs`; `None` — brak monitora).
    pub signals: Option<Arc<dyn platform_contract::SystemSignalsPort>>,
    /// Obserwacja katalogów wyzwalaczy.
    pub dir_watch: Option<Arc<dyn platform_contract::DirWatchPort>>,
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
            signals: super::signals::signals(options),
            dir_watch: super::signals::dir_watch(options),
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

pub(crate) async fn cost_meter(
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
