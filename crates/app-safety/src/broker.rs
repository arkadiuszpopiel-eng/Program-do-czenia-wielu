//! `alfa-broker`: złożenie usługi Brokera z portów platformy.
//!
//! Produkcja: usługa Windows `AlfaBroker` z plikiem konfiguracji (`--config`, JSON
//! `ServiceConfig`, tworzony przy instalacji — bramka ludzka #10: konto usługi z
//! `SeTcbPrivilege`, katalog danych, ścieżki obrazów). Tryb deweloperski `--console`: bieżące
//! konto, potok `alfa-broker-dev`, Broker-UI „jak wywołujący” (bez UIPI — ostrzeżenie w logu).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use compliance_contract::PathEnv;
use platform_contract::{
    IntegrityLevel, LaunchIntegrity, PeerRequirement, PrivateDirPort, ProcessPort,
    SessionLauncherPort, Sid, StopSignal, UnverifiedSignatures,
};
use platform_windows_impl::{JobLimits, WinProcesses};
use platform_windows_kernel_impl::{WinKernel, WinSessionLauncher};
use safety_broker_contract::KernelPolicy;
use safety_broker_impl::service::{
    BrokerService, RoleBinding, RoleBindings, ServiceConfig, ServicePorts, UiLaunchConfig,
    open_audit,
};
use safety_broker_impl::{BrokerConfig, BrokerEngine, KeyMode};
use watchdog_contract::{Clock, SystemClock};

use crate::ChildLauncher;

/// Nazwa usługi Windows.
pub const SERVICE_NAME: &str = "AlfaBroker";
/// Potok trybu deweloperskiego.
pub const DEV_PIPE: &str = "alfa-broker-dev";
/// Obrazy jądra Alfy (powłoka Tauri / przyszły proces jądra).
pub const CORE_IMAGES: [&str; 3] = ["alfa-desktop.exe", "Alfa.exe", "alfa-core.exe"];

fn binding(
    user: &Sid,
    dir: &Path,
    images: &[&str],
    min: IntegrityLevel,
    enroll: bool,
) -> Option<RoleBinding> {
    Some(RoleBinding {
        requirement: PeerRequirement {
            users: vec![user.clone()],
            min_integrity: min,
            images: images.iter().map(|i| dir.join(i)).collect(),
            signer: None,
            session: None,
        },
        enroll,
    })
}

/// Konfiguracja trybu deweloperskiego: wszystko na bieżącym koncie, obrazy obok `alfa-broker`.
pub fn dev_config(
    exe_dir: &Path,
    user: Sid,
    user_profile: &str,
    data_dir: PathBuf,
) -> ServiceConfig {
    let medium = IntegrityLevel::Medium;
    ServiceConfig {
        pipe_name: DEV_PIPE.into(),
        broker_user: user.clone(),
        client_users: vec![user.clone()],
        data_dir,
        user_profile: user_profile.into(),
        bindings: RoleBindings {
            core: binding(&user, exe_dir, &CORE_IMAGES, medium, true),
            agent: None,
            broker_ui: binding(&user, exe_dir, &["alfa-broker-ui.exe"], medium, false),
            watchdog: binding(&user, exe_dir, &["alfa-watchdog.exe"], medium, true),
        },
        broker_ui: Some(UiLaunchConfig {
            image: exe_dir.join("alfa-broker-ui.exe"),
            args: Vec::new(),
            integrity: LaunchIntegrity::AsCaller,
            credential_ttl_ms: 24 * 60 * 60 * 1000,
            restart_backoff_ms: 1_000,
        }),
        dev_mode: true,
    }
}

/// Wczytuje konfigurację z pliku JSON i ją waliduje.
pub fn load_config(path: &Path) -> Result<ServiceConfig, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let config: ServiceConfig =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    config.validate()?;
    Ok(config)
}

/// Polityka bazowa Jądra dla profilu właściciela; katalog danych Brokera = ścieżka Jądra.
pub fn policy_for(config: &ServiceConfig) -> Result<KernelPolicy, String> {
    let data_dir = config.data_dir.to_string_lossy();
    KernelPolicy::baseline(&config.user_profile, &data_dir).map_err(|e| e.to_string())
}

/// Silnik Brokera z Audytem w katalogu prywatnym (kotwica obok).
pub fn build_engine(
    config: &ServiceConfig,
    policy: KernelPolicy,
    dirs: &dyn PrivateDirPort,
    processes: Arc<dyn ProcessPort>,
    clock: Arc<dyn Clock>,
) -> Result<Arc<BrokerEngine>, String> {
    let audit = open_audit(
        dirs,
        &config.data_dir,
        &config.broker_user,
        clock.clone(),
        None,
    )?;
    let broker = BrokerConfig {
        policy,
        env: PathEnv::windows_profile(&config.user_profile),
        key_mode: KeyMode::Random,
    };
    BrokerEngine::new(broker, clock, audit, processes)
        .map(Arc::new)
        .map_err(|e| e.to_string())
}

/// Port uruchamiania Broker-UI zgodny z konfiguracją (wysoka integralność → WTS + token,
/// tryb deweloperski → proces potomny).
pub fn launcher_for(config: &ServiceConfig) -> Arc<dyn SessionLauncherPort> {
    match config.broker_ui.as_ref().map(|u| u.integrity) {
        Some(LaunchIntegrity::AsCaller) => Arc::new(ChildLauncher::default()),
        _ => Arc::new(WinSessionLauncher::default()),
    }
}

/// Uruchamia usługę do zatrzymania (`stop`); wspólne dla trybu usługi i konsoli.
pub fn run(config: ServiceConfig, stop: &StopSignal) -> Result<(), String> {
    if config.dev_mode {
        eprintln!("[alfa-broker] TRYB DEWELOPERSKI — bez osobnego konta i bez UIPI dla Broker-UI");
        tracing::warn!("tryb deweloperski Brokera — bez osobnego konta i bez UIPI dla Broker-UI");
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .map_err(|e| e.to_string())?;
    let processes: Arc<dyn ProcessPort> = Arc::new(WinProcesses::new(JobLimits::default()));
    let policy = policy_for(&config)?;
    let engine = build_engine(
        &config,
        policy,
        &WinKernel,
        processes,
        Arc::new(SystemClock),
    )?;
    let ports = ServicePorts {
        pipes: Arc::new(WinKernel),
        identity: Arc::new(WinKernel),
        signatures: Arc::new(UnverifiedSignatures),
        launcher: launcher_for(&config),
    };
    let service = BrokerService::new(
        engine,
        config,
        ports,
        runtime.handle().clone(),
        stop.clone(),
    )?;
    let service = Arc::new(service);
    let _threads = service.start()?;
    while !stop.wait(Duration::from_secs(3_600)) {}
    Ok(())
}
