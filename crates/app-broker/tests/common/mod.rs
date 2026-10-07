//! Wspólna atrapa łańcucha procesów: usługa Brokera (`safety-broker-impl::service`) na potokach
//! z ACL (`platform-fake` albo gniazda Unix), Broker-UI jako wątek „uruchamiany” przez usługę
//! (bilet na stdin → `PipeLink` → skryptowany właściciel z `broker-ui-fake`), aplikacja jako
//! klient roli `Core` przez `app-broker`.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod breaker;
pub mod processes;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_api::dto::BrokerMode;
use app_broker::link::{BrokerLink, LinkConfig, ServerCheck};
use app_broker::{KernelStatus, RemoteKernel};
use broker_ui_fake::{Script, ScriptedBrokerUi};
use broker_ui_impl::PipeLink;
use broker_ui_impl::driver::cycle;
use compliance_contract::PathEnv;
use platform_contract::{
    IntegrityLevel, LaunchIntegrity, PeerIdentity, PeerRequirement, PlatformError,
    ProcessIdentityPort, SecurePipePort, SessionLaunch, SessionLauncherPort, Sid, SignatureStatus,
    StopSignal, UnverifiedSignatures,
};
use safety_broker_contract::KernelPolicy;
use safety_broker_contract::contract_tests::PROFILE;
use safety_broker_contract::ipc_blocking::UiLaunchTicket;
use safety_broker_impl::audit::MemoryAudit;
use safety_broker_impl::service::{
    BrokerService, RoleBinding, RoleBindings, ServiceConfig, ServicePorts, UiLaunchConfig,
};
use safety_broker_impl::{BrokerConfig, BrokerEngine, KeyMode};
use watchdog_contract::{Clock, ManualClock};

pub const BROKER: &str = "S-1-5-80-9-9-9-9-9";
pub const USER: &str = "S-1-5-21-5-6-7-1001";
pub const DIR: &str = r"C:\Program Files\Alfa";
pub const PIPE: &str = "alfa-broker";
pub const SERVER_PID: u32 = 1;
pub const CORE_PID: u32 = 10;
pub const UI_PID: u32 = 13;
pub const STRANGER_PID: u32 = 66;

/// Tożsamość procesu w atrapie (usługa: sesja 0, integralność systemowa).
pub fn ident(pid: u32, user: &str, image: &str, integrity: IntegrityLevel) -> PeerIdentity {
    PeerIdentity {
        pid,
        image: Path::new(DIR).join(image),
        user: Sid::parse(user).unwrap(),
        integrity,
        session: u32::from(integrity < IntegrityLevel::System),
        signature: SignatureStatus::NotVerified,
    }
}

/// Procesy: usługa, aplikacja, Broker-UI, obcy proces użytkownika.
pub fn identities() -> Vec<PeerIdentity> {
    vec![
        ident(
            SERVER_PID,
            BROKER,
            "alfa-broker.exe",
            IntegrityLevel::System,
        ),
        ident(CORE_PID, USER, "alfa-desktop.exe", IntegrityLevel::Medium),
        ident(UI_PID, USER, "alfa-broker-ui.exe", IntegrityLevel::High),
        ident(STRANGER_PID, USER, "obcy.exe", IntegrityLevel::Medium),
    ]
}

fn binding(image: &str, min: IntegrityLevel, enroll: bool) -> Option<RoleBinding> {
    Some(RoleBinding {
        requirement: PeerRequirement {
            users: vec![Sid::parse(USER).unwrap()],
            min_integrity: min,
            images: vec![Path::new(DIR).join(image)],
            signer: None,
            session: None,
        },
        enroll,
    })
}

/// Konfiguracja usługi jak po instalacji (bramka #10), z Broker-UI albo bez.
pub fn config(data: &Path, with_ui: bool) -> ServiceConfig {
    ServiceConfig {
        pipe_name: PIPE.into(),
        broker_user: Sid::parse(BROKER).unwrap(),
        client_users: vec![Sid::parse(USER).unwrap()],
        data_dir: data.to_path_buf(),
        user_profile: PROFILE.into(),
        bindings: RoleBindings {
            core: binding("alfa-desktop.exe", IntegrityLevel::Medium, true),
            agent: None,
            broker_ui: binding("alfa-broker-ui.exe", IntegrityLevel::High, false),
            watchdog: binding("alfa-watchdog.exe", IntegrityLevel::Medium, true),
        },
        broker_ui: with_ui.then(|| UiLaunchConfig {
            image: Path::new(DIR).join("alfa-broker-ui.exe"),
            args: Vec::new(),
            integrity: LaunchIntegrity::UserSessionHigh,
            credential_ttl_ms: 24 * 60 * 60 * 1000,
            restart_backoff_ms: 100,
        }),
        dev_mode: false,
    }
}

/// Silnik usługi (Audyt w pamięci, procesy-atrapy).
pub fn engine(policy: KernelPolicy, clock: Arc<ManualClock>) -> Arc<BrokerEngine> {
    let config = BrokerConfig {
        policy,
        env: PathEnv::windows_profile(PROFILE),
        key_mode: KeyMode::Random,
    };
    let processes = Arc::new(safety_broker_fake::FakeProcesses::default());
    Arc::new(BrokerEngine::new(config, clock, Arc::new(MemoryAudit::default()), processes).unwrap())
}

/// Działająca usługa Brokera.
pub struct Service {
    pub engine: Arc<BrokerEngine>,
    pub clock: Arc<ManualClock>,
    pub stop: StopSignal,
    _service: Arc<BrokerService>,
    rt: Option<tokio::runtime::Runtime>,
    _dir: tempfile::TempDir,
}

impl Drop for Service {
    fn drop(&mut self) {
        self.stop.stop();
        // Bez blokowania — usługa bywa sprzątana wewnątrz testu asynchronicznego.
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

/// Uruchamia usługę na porcie potoków widzianym jako proces `SERVER_PID`.
pub fn service(
    pipes: Arc<dyn SecurePipePort>,
    identity: Arc<dyn ProcessIdentityPort>,
    policy: KernelPolicy,
    clock: Arc<ManualClock>,
    launcher: Option<Arc<dyn SessionLauncherPort>>,
) -> Service {
    service_with(pipes, identity, policy, clock, launcher, |_| {})
}

/// Jak [`service`], z modyfikacją konfiguracji (tryb przenośny: potok i konto).
pub fn service_with(
    pipes: Arc<dyn SecurePipePort>,
    identity: Arc<dyn ProcessIdentityPort>,
    policy: KernelPolicy,
    clock: Arc<ManualClock>,
    launcher: Option<Arc<dyn SessionLauncherPort>>,
    tweak: impl FnOnce(&mut ServiceConfig),
) -> Service {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(policy, clock.clone());
    let mut config = config(&dir.path().join("broker"), launcher.is_some());
    tweak(&mut config);
    let ports = ServicePorts {
        pipes,
        identity,
        signatures: Arc::new(UnverifiedSignatures),
        launcher: launcher.unwrap_or_else(|| Arc::new(NeverLaunch)),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let stop = StopSignal::new();
    let service = BrokerService::new(
        engine.clone(),
        config,
        ports,
        rt.handle().clone(),
        stop.clone(),
    )
    .unwrap();
    let service = Arc::new(service);
    service.start().unwrap();
    Service {
        engine,
        clock,
        stop,
        _service: service,
        rt: Some(rt),
        _dir: dir,
    }
}

struct NeverLaunch;

impl SessionLauncherPort for NeverLaunch {
    fn launch(&self, _spec: &SessionLaunch) -> Result<u32, PlatformError> {
        Err(PlatformError::Unsupported("bez Broker-UI".into()))
    }
    fn is_running(&self, _pid: u32) -> bool {
        false
    }
}

/// „Uruchamianie” Broker-UI przez usługę: wątek z biletem ze stdin, łączem `PipeLink` jako
/// proces `UI_PID` i skryptowanym właścicielem (`broker-ui-fake`).
pub struct UiThreadLauncher {
    pipes: Arc<dyn SecurePipePort>,
    identity: Arc<dyn ProcessIdentityPort>,
    clock: Arc<ManualClock>,
    script: Arc<Mutex<Script>>,
    alive: Arc<AtomicBool>,
    stop: StopSignal,
}

impl UiThreadLauncher {
    pub fn new(
        pipes: Arc<dyn SecurePipePort>,
        identity: Arc<dyn ProcessIdentityPort>,
        clock: Arc<ManualClock>,
        script: Script,
    ) -> Arc<Self> {
        Arc::new(Self {
            pipes,
            identity,
            clock,
            script: Arc::new(Mutex::new(script)),
            alive: Arc::new(AtomicBool::new(false)),
            stop: StopSignal::new(),
        })
    }

    /// Zmienia reakcję „właściciela” (dla kolejnych kart).
    pub fn set_script(&self, script: Script) {
        *self.script.lock().unwrap() = script;
    }

    /// Czy okno działa (połączone z Brokerem).
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.stop.stop();
    }
}

impl SessionLauncherPort for UiThreadLauncher {
    fn launch(&self, spec: &SessionLaunch) -> Result<u32, PlatformError> {
        let ticket: UiLaunchTicket = serde_json::from_slice(&spec.stdin)
            .map_err(|e| PlatformError::Io(format!("bilet: {e}")))?;
        let (pipes, identity) = (self.pipes.clone(), self.identity.clone());
        let (clock, script) = (self.clock.clone(), self.script.clone());
        let (alive, stop) = (self.alive.clone(), self.stop.clone());
        alive.store(true, Ordering::SeqCst);
        std::thread::spawn(move || {
            let link = PipeLink::connect(&*pipes, &*identity, &ticket, UI_PID);
            if let Ok(mut link) = link {
                let mut ui = ScriptedBrokerUi::new();
                while !stop.wait(Duration::from_millis(5)) {
                    ui.set_default(Some(*script.lock().unwrap()));
                    if cycle(&mut ui, &mut link, clock.now_ms(), 0, true).is_err() {
                        break;
                    }
                }
            }
            alive.store(false, Ordering::SeqCst);
        });
        Ok(UI_PID)
    }

    fn is_running(&self, _pid: u32) -> bool {
        self.alive()
    }
}

/// Aplikacja: łącze roli `Core` jako proces `CORE_PID` ze sprawdzeniem usługi.
pub fn app_kernel(
    pipes: Arc<dyn SecurePipePort>,
    identity: Arc<dyn ProcessIdentityPort>,
    with_ui: bool,
) -> (RemoteKernel, Arc<BrokerLink>) {
    let status = Arc::new(KernelStatus::new(BrokerMode::Service, with_ui));
    let mut config = LinkConfig::new(PIPE);
    config.call_timeout = Duration::from_secs(5);
    let link = BrokerLink::new(pipes, identity, config, status);
    let check = ServerCheck::Service {
        user: Some(Sid::parse(BROKER).unwrap()),
    };
    let mut last = None;
    for _ in 0..200 {
        match link.connect(&check) {
            Ok(()) => return (RemoteKernel::new(link.clone()), link),
            Err(e) => last = Some(e),
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("aplikacja nie połączyła się z Brokerem: {last:?}");
}

/// Czeka na warunek (wątki usługi i Broker-UI działają naprawdę równolegle).
pub fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
    for _ in 0..1_000 {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("nie doczekano: {what}");
}

/// Katalog obrazów w atrapie.
pub fn image(name: &str) -> PathBuf {
    Path::new(DIR).join(name)
}
