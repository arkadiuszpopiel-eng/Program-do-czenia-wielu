//! Budowa modułów podpiętych po F1: rezydencja modeli, model lokalny, Router, klasyfikator ryzyka,
//! Broker w procesie (tryb deweloperski), dziennik cofania, transfer, audio, TTS, aktualizacje.
//! Awaria budowy takiego modułu nie zatrzymuje aplikacji: moduł zostaje „niepodłączony"
//! (port zwraca czytelny błąd, Broker — bezpieczną odmowę), a rejestr pokazuje go jako
//! niezdrowy.

use std::collections::BTreeMap;
use std::sync::Arc;

use accounts_hub_contract::SecretStore;
use compliance_contract::Compliance;
use core_bus_contract::EventBus;
use core_registry_contract::HealthStatus;
use cost_meter_contract::CostMeter;
use cost_meter_impl::CostMeterService;
use device_profile_contract::DeviceProfile;
use memory_contract::{MemoryService, PrivacyOracle};
use model_residency_contract::Residency;
use model_residency_impl::ResidencyModule;
use providers_local_impl::LocalModule;
use risk_classifier_contract::RiskPolicy;
use risk_classifier_impl::TableClassifier;
use router_impl::{CostMeterGate, RouterCore, RouterModule};
use safety_broker_contract::KernelPolicy;
use safety_broker_impl::audit::{BrokerAuditWriter, FileAnchorStore};
use safety_broker_impl::{BrokerConfig, BrokerEngine, KeyMode};
use sessions_contract::Sessions;
use sessions_impl::SqliteSessions;
use transfer_contract::{Category, DocumentStore, Limits, MachineInfo, SystemClock, TransferPorts};
use transfer_impl::{DirDocumentStore, DirFilter, TransferConfig, UuidIds, ZipTransfer};
use undo_journal_contract::UndoLimits;
use undo_journal_impl::UndoService;
use updater_impl::{FsUpdater, UpdaterConfig};
use voice_audio_impl::VoiceAudioModule;
use voice_tts_contract::Tts;
use voice_tts_impl::VoiceTtsModule;

use crate::compose::{HealthSlot, started};
use crate::error::AppError;
use crate::options::{AppOptions, AppPaths};
use crate::parts::Kernel;
use app_modules::broker::{dev_dir, path_env_for};
use app_modules::route::{Routers, local};

/// Zależności z modułów podstawowych.
pub(crate) struct Deps<'a> {
    pub paths: &'a AppPaths,
    pub options: &'a AppOptions,
    pub bus: &'a Arc<dyn EventBus>,
    pub kernel: &'a Kernel,
    pub compliance: Option<Arc<dyn Compliance>>,
    pub costs: Option<Arc<CostMeterService>>,
    pub sessions: Option<Arc<SqliteSessions>>,
    /// Pamięć F7 i prywatność sesji (dokumenty `memory` w paczce `.alfa`).
    pub memory: Option<(Arc<dyn MemoryService>, Arc<dyn PrivacyOracle>)>,
    pub slot: Option<&'a HealthSlot>,
}

/// Moduły podpięte po F1.
#[derive(Default)]
pub(crate) struct Extra {
    pub residency: Option<Arc<ResidencyModule>>,
    pub local: Option<Arc<LocalModule>>,
    pub routers: Option<Routers>,
    pub router: Option<Arc<RouterModule>>,
    pub classifier: Option<Arc<TableClassifier>>,
    pub broker: Option<Arc<BrokerEngine>>,
    /// Wykonanie poleceń narzędzi i zabijanie ich przez Brokera (jedna instancja).
    pub exec: Option<Arc<dyn platform_contract::ExecPort>>,
    pub undo: Option<Arc<UndoService>>,
    pub transfer: Option<Arc<ZipTransfer>>,
    pub audio: Option<Arc<VoiceAudioModule>>,
    pub tts_module: Option<Arc<VoiceTtsModule>>,
    pub tts: Option<Arc<dyn Tts>>,
    pub updater: Option<Arc<FsUpdater>>,
    /// Gotowość narzędzi i runtime agentek — ustalana po złożeniu stosu agentek (moduły
    /// `agent-runtime`, `tools-*` startują w kolejności przed dziennikiem cofania).
    pub agents_ready: Arc<std::sync::OnceLock<Result<(), String>>>,
    /// Usługi modułów trzymane przez cały czas życia rdzenia (ich zdrowie czyta rejestr).
    pub keep: Vec<Arc<dyn std::any::Any + Send + Sync>>,
}

/// System plików narzędzi i dziennika cofania (ten sam port — cofnięcie widzi te same pliki).
pub(crate) fn platform_fs(options: &AppOptions) -> Arc<dyn platform_contract::FsPort> {
    options
        .fs
        .clone()
        .unwrap_or_else(|| Arc::new(platform_windows_impl::WindowsPlatform::default()))
}

/// Moduł zdrowy bez usługi (budowany leniwie albo w procesie powłoki).
pub(crate) fn healthy(slot: Option<&HealthSlot>) {
    if let Some(slot) = slot {
        let _ = slot.set(Arc::new(|| HealthStatus::Healthy));
    }
}

pub(crate) fn unhealthy(slot: Option<&HealthSlot>, id: &str, e: &AppError) {
    tracing::error!(modul = id, error = %e, "moduł niepodłączony — budowa nie powiodła się");
    if let Some(slot) = slot {
        let message = e.message.clone();
        let _ = slot.set(Arc::new(move || HealthStatus::Unhealthy(message.clone())));
    }
}

fn err(id: &str) -> impl Fn(String) -> AppError + '_ {
    move |e| AppError::internal(format!("{id}: {e}"))
}

impl Extra {
    /// Buduje moduł `id`; `false` = to nie jest moduł tej części.
    pub async fn build(&mut self, id: &str, deps: Deps<'_>) -> bool {
        let result = match id {
            "model-residency" => self.residency(&deps).await,
            "providers-local" => self.local(&deps).await,
            "router" => self.router(&deps).await,
            "risk-classifier" => self.classifier(&deps).await,
            "safety-broker" => self.broker(&deps),
            "undo-journal" => self.undo(&deps).await,
            "transfer" => self.transfer(&deps).await,
            "voice-audio" => self.audio(&deps).await,
            "voice-tts" => self.tts(&deps).await,
            "updater" => self.updater(&deps).await,
            // Narzędzia i runtime agentek składa `Extra::agents` po zbudowaniu Brokera
            // i dziennika cofania (wymagane przez manifesty — są wcześniej w kolejności).
            "tools-fs" | "tools-shell" | "tools-clipboard" | "agent-runtime" => {
                self.agents_ready(id, &deps)
            }
            _ => return false,
        };
        if let Err(e) = result {
            unhealthy(deps.slot, id, &e);
        }
        true
    }

    fn device(deps: &Deps<'_>) -> Result<Arc<dyn DeviceProfile>, AppError> {
        deps.kernel
            .device
            .clone()
            .ok_or_else(|| AppError::internal("brak profilu urządzenia"))
    }

    async fn residency(&mut self, deps: &Deps<'_>) -> Result<(), AppError> {
        let module = local::residency(&Self::device(deps)?)?;
        self.residency = Some(started(module, deps.bus, deps.slot).await?);
        Ok(())
    }

    async fn local(&mut self, deps: &Deps<'_>) -> Result<(), AppError> {
        let residency = self
            .residency
            .as_ref()
            .map(|r| r.manager() as Arc<dyn Residency>);
        let module = local::provider_module(deps.paths, &Self::device(deps)?, residency)?;
        self.local = Some(started(module, deps.bus, deps.slot).await?);
        Ok(())
    }

    async fn router(&mut self, deps: &Deps<'_>) -> Result<(), AppError> {
        let costs = deps
            .costs
            .clone()
            .ok_or_else(|| AppError::internal("router: brak cost-meter"))?;
        let meter: Arc<dyn CostMeter> = costs;
        let gate = Arc::new(CostMeterGate::new(
            meter,
            Arc::new(cost_meter_impl::SystemClock),
        ));
        let core = || {
            Arc::new(RouterCore::new(
                deps.compliance.clone(),
                Some(gate.clone() as Arc<dyn router_contract::BudgetGate>),
            ))
        };
        let routers = Routers {
            hybrid: core(),
            cloud: core(),
            local: core(),
        };
        if let Some(local) = &self.local {
            routers.register(local.provider(), router_contract::RouteKind::Local);
        }
        for (provider, kind) in &deps.options.providers {
            routers.register(provider.clone(), *kind);
        }
        app_modules::route::config::apply(&deps.kernel.config, &routers).await;
        routers.forward_secondary(deps.bus);
        let module =
            RouterModule::new(routers.hybrid.clone()).map_err(|e| err("router")(e.to_string()))?;
        self.router = Some(started(module, deps.bus, deps.slot).await?);
        self.routers = Some(routers);
        Ok(())
    }

    async fn classifier(&mut self, deps: &Deps<'_>) -> Result<(), AppError> {
        let module = TableClassifier::new(RiskPolicy::default())
            .map_err(|e| err("risk-classifier")(e.to_string()))?;
        self.classifier = Some(started(module, deps.bus, deps.slot).await?);
        Ok(())
    }

    /// Broker w procesie: polityka bazowa profilu, Audyt w pliku z łańcuchem i kotwicą.
    fn broker(&mut self, deps: &Deps<'_>) -> Result<(), AppError> {
        let (profile, env) = path_env_for(&deps.paths.user_root);
        let dir = dev_dir(&deps.paths.local);
        let policy = KernelPolicy::baseline(&profile, &dir.to_string_lossy())
            .or_else(|_| KernelPolicy::baseline(&profile, r"C:\ProgramData\AlfaBroker"))
            .map_err(|e| err("safety-broker")(e.to_string()))?;
        let clock = Arc::new(watchdog_contract::SystemClock);
        let audit = BrokerAuditWriter::open(
            dir.join("audit.ndjson"),
            Arc::new(FileAnchorStore::new(dir.join("anchor.json"))),
            Arc::new(core_log_contract::RegexRedactor::default()),
            clock.clone(),
            "broker-dev",
            None,
        )
        .map_err(|e| err("safety-broker: Audyt")(e.to_string()))?;
        let config = BrokerConfig {
            policy,
            env,
            key_mode: KeyMode::Random,
        };
        // Procesy narzędzi (`shell_run`) zabija ten sam port (ta sama tablica uchwytów Job
        // Objects), który je uruchomił — jedna instancja dla Brokera i narzędzi agentek.
        let exec: Arc<dyn platform_contract::ExecPort> = match &deps.options.exec {
            Some(exec) => exec.clone(),
            None => Arc::new(platform_windows_impl::WindowsPlatform::default()),
        };
        self.exec = Some(exec.clone());
        let processes: Arc<dyn platform_contract::ProcessPort> = exec;
        let mut engine = BrokerEngine::new(config, clock, Arc::new(audit), processes)
            .map_err(|e| err("safety-broker")(e.to_string()))?
            .with_bus(deps.bus.clone());
        if let Some(c) = &self.classifier {
            engine = engine.with_classifier(c.clone());
        }
        self.broker = Some(Arc::new(engine));
        if let Some(slot) = deps.slot {
            let _ = slot.set(Arc::new(|| HealthStatus::Healthy));
        }
        Ok(())
    }

    async fn undo(&mut self, deps: &Deps<'_>) -> Result<(), AppError> {
        let fs = platform_fs(deps.options);
        let clock = Arc::new(|| u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0));
        let boot = u64::try_from(chrono::Utc::now().timestamp_micros()).unwrap_or(0);
        let service = UndoService::open(
            fs,
            deps.paths.local.join("undo-store"),
            UndoLimits::default(),
            clock,
            boot,
        )
        .map_err(|e| err("undo-journal")(e.to_string()))?;
        self.undo = Some(started(service, deps.bus, deps.slot).await?);
        Ok(())
    }

    async fn transfer(&mut self, deps: &Deps<'_>) -> Result<(), AppError> {
        let sessions: Arc<dyn Sessions> = deps
            .sessions
            .clone()
            .ok_or_else(|| AppError::internal("transfer: brak sessions"))?;
        let device = Self::device(deps)?;
        let profile = device.current();
        let hw = serde_json::to_value(device.recommend().class)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.replace('_', "-")))
            .unwrap_or_else(|| "unknown".into());
        let config = &deps.paths.config;
        let store = |root: std::path::PathBuf, filter: DirFilter| {
            Arc::new(DirDocumentStore::new(root, filter)) as Arc<dyn DocumentStore>
        };
        let mut documents = BTreeMap::new();
        documents.insert(
            Category::ConfigCommon,
            store(config.clone(), DirFilter::flat(&["toml"])),
        );
        documents.insert(
            Category::ConfigMachine,
            store(config.join("machine"), DirFilter::flat(&["toml"])),
        );
        documents.insert(
            Category::Logs,
            store(deps.paths.logs(), DirFilter::flat(&["ndjson"])),
        );
        if let Some((memory, privacy)) = &deps.memory {
            let docs = app_memory::MemoryDocuments::new(memory.clone(), privacy.clone());
            documents.insert(Category::Memory, Arc::new(docs) as Arc<dyn DocumentStore>);
        }
        let secrets: Arc<dyn SecretStore> = deps.kernel.secrets.clone();
        let ports = TransferPorts {
            sessions: Some(sessions),
            documents,
            secrets: Some(secrets),
            machine: MachineInfo {
                id: profile.machine_id.as_str().to_owned(),
                name: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Ten komputer".into()),
                os: format!("{} {}", profile.os.name, profile.os.version)
                    .trim()
                    .to_owned(),
                hw_class: hw,
            },
            app_version: semver::Version::parse(&deps.options.app_version)
                .unwrap_or_else(|_| semver::Version::new(0, 0, 0)),
            workdir_root: Some(deps.paths.workdirs()),
            clock: Arc::new(SystemClock),
            ids: Arc::new(UuidIds),
            limits: Limits::default(),
        };
        let module = ZipTransfer::new(ports, TransferConfig::new(deps.paths.snapshots()))
            .map_err(|e| err("transfer")(e.to_string()))?;
        self.transfer = Some(started(module, deps.bus, deps.slot).await?);
        Ok(())
    }

    async fn audio(&mut self, deps: &Deps<'_>) -> Result<(), AppError> {
        let io = deps
            .options
            .audio
            .clone()
            .unwrap_or_else(voice_audio_impl::system_audio);
        let module = VoiceAudioModule::new(io).map_err(|e| err("voice-audio")(e.to_string()))?;
        self.audio = Some(started(module, deps.bus, deps.slot).await?);
        Ok(())
    }

    async fn tts(&mut self, deps: &Deps<'_>) -> Result<(), AppError> {
        let module = VoiceTtsModule::new().map_err(|e| err("voice-tts")(e.to_string()))?;
        self.tts_module = Some(started(module, deps.bus, deps.slot).await?);
        self.tts = match &deps.options.tts {
            Some(tts) => Some(tts.clone()),
            None => app_modules::tts::engines(deps.paths)?,
        };
        Ok(())
    }

    /// Zdrowie narzędzi i runtime agentek: „nieuruchomione" do złożenia stosu agentek, potem
    /// zdrowe albo niesprawne (brak Brokera lub dziennika cofania) — bez fałszywych awarii
    /// zależnych od kolejności startu.
    fn agents_ready(&mut self, id: &str, deps: &Deps<'_>) -> Result<(), AppError> {
        if let Some(slot) = deps.slot {
            let (ready, id) = (self.agents_ready.clone(), id.to_owned());
            let _ = slot.set(Arc::new(move || match ready.get() {
                None => HealthStatus::NotStarted,
                Some(Ok(())) => HealthStatus::Healthy,
                Some(Err(why)) => HealthStatus::Unhealthy(format!("{id}: {why}")),
            }));
        }
        Ok(())
    }

    async fn updater(&mut self, deps: &Deps<'_>) -> Result<(), AppError> {
        let module = FsUpdater::new(UpdaterConfig::new(&deps.paths.local))
            .map_err(|e| err("updater")(e.to_string()))?;
        self.updater = Some(started(module, deps.bus, deps.slot).await?);
        Ok(())
    }
}
