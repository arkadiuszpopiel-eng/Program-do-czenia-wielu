//! Usługa Brokera na atrapach platformy: potok z ACL, wiązanie ról z tożsamością procesu
//! (konto, integralność, obraz), zapis jądra/watchdoga po tożsamości, Broker-UI tylko z biletem
//! i wysoką integralnością, przejęcie nazwy potoku, nadzór Broker-UI, Audyt w katalogu prywatnym.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use platform_contract::{
    IntegrityLevel, LaunchIntegrity, PeerIdentity, PeerRequirement, PipeSecurity, SecurePipePort,
    Sid, SignatureStatus, StopSignal, UnverifiedSignatures,
};
use platform_fake::{FakeLauncher, FakePipes, FakePrivateDirs};
use safety_broker_contract::ipc::{ClientCredential, ClientRole, Hello, Request, Response};
use safety_broker_contract::ipc_blocking::{BlockingClient, BlockingError, UiLaunchTicket};
use safety_broker_impl::BrokerEngine;
use safety_broker_impl::audit::MemoryAudit;
use safety_broker_impl::service::{
    BrokerService, RoleBinding, RoleBindings, ServiceConfig, ServicePorts, UiLaunchConfig,
    UiSupervisor, open_audit,
};
use watchdog_contract::{Clock, ManualClock};

const BROKER: &str = "S-1-5-80-11-22-33-44-55";
const USER: &str = "S-1-5-21-1-2-3-1001";
const OTHER: &str = "S-1-5-21-1-2-3-1002";
const CORE_EXE: &str = r"C:\Program Files\Alfa\alfa-core.exe";
const UI_EXE: &str = r"C:\Program Files\Alfa\alfa-broker-ui.exe";
const WD_EXE: &str = r"C:\Program Files\Alfa\alfa-watchdog.exe";

fn sid(s: &str) -> Sid {
    Sid::parse(s).unwrap()
}

fn ident(pid: u32, user: &str, image: &str, integrity: IntegrityLevel) -> PeerIdentity {
    PeerIdentity {
        pid,
        image: PathBuf::from(image),
        user: sid(user),
        integrity,
        session: 1,
        signature: SignatureStatus::NotVerified,
    }
}

fn binding(image: &str, min: IntegrityLevel, enroll: bool) -> Option<RoleBinding> {
    Some(RoleBinding {
        requirement: PeerRequirement {
            users: vec![sid(USER)],
            min_integrity: min,
            images: vec![PathBuf::from(image)],
            signer: None,
            session: None,
        },
        enroll,
    })
}

fn config() -> ServiceConfig {
    ServiceConfig {
        pipe_name: "alfa-broker-t".into(),
        broker_user: sid(BROKER),
        client_users: vec![sid(USER)],
        data_dir: PathBuf::from("unused"),
        user_profile: r"C:\Users\ala".into(),
        bindings: RoleBindings {
            core: binding(CORE_EXE, IntegrityLevel::Medium, true),
            agent: None,
            broker_ui: binding(UI_EXE, IntegrityLevel::High, false),
            watchdog: binding(WD_EXE, IntegrityLevel::Medium, true),
        },
        broker_ui: Some(UiLaunchConfig {
            image: PathBuf::from(UI_EXE),
            args: vec![],
            integrity: LaunchIntegrity::UserSessionHigh,
            credential_ttl_ms: 86_400_000,
            restart_backoff_ms: 1_000,
        }),
        dev_mode: false,
    }
}

struct Rig {
    sys: FakePipes,
    launcher: Arc<FakeLauncher>,
    engine: Arc<BrokerEngine>,
    audit: Arc<MemoryAudit>,
    clock: Arc<ManualClock>,
    service: Arc<BrokerService>,
    _rt: tokio::runtime::Runtime,
}

fn rig() -> Rig {
    let sys = FakePipes::new(ident(
        1,
        BROKER,
        r"C:\Alfa\alfa-broker.exe",
        IntegrityLevel::System,
    ));
    sys.register(ident(10, USER, CORE_EXE, IntegrityLevel::Medium));
    sys.register(ident(
        11,
        USER,
        r"C:\Users\ala\Downloads\x.exe",
        IntegrityLevel::Medium,
    ));
    sys.register(ident(12, USER, UI_EXE, IntegrityLevel::Medium));
    sys.register(ident(13, USER, UI_EXE, IntegrityLevel::High));
    sys.register(ident(14, OTHER, CORE_EXE, IntegrityLevel::Medium));
    sys.register(ident(15, USER, WD_EXE, IntegrityLevel::Medium));
    let (engine, audit, clock) = common::engine();
    let engine = Arc::new(engine);
    let launcher = Arc::new(FakeLauncher::new());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ports = ServicePorts {
        pipes: Arc::new(sys.clone()),
        identity: Arc::new(sys.clone()),
        signatures: Arc::new(UnverifiedSignatures),
        launcher: launcher.clone(),
    };
    let service = BrokerService::new(
        engine.clone(),
        config(),
        ports,
        rt.handle().clone(),
        StopSignal::new(),
    )
    .unwrap();
    Rig {
        sys,
        launcher,
        engine,
        audit,
        clock,
        service: Arc::new(service),
        _rt: rt,
    }
}

fn hello(role: ClientRole, credential: Option<ClientCredential>, pid: u32) -> Hello {
    Hello {
        protocol: 1,
        credential: credential.unwrap_or(ClientCredential {
            client_id: format!("{role:?}"),
            role,
            expires_at_ms: 0,
            mac: String::new(),
        }),
        pid,
        sid: None,
        image: None,
    }
}

/// Łączy proces `pid` z rolą; serwer obsługuje połączenie w osobnym wątku.
fn session(
    r: &Rig,
    listener: &mut Box<dyn platform_contract::PipeListener>,
    pid: u32,
    h: Hello,
) -> Result<BlockingClient<Box<dyn platform_contract::PipeConnection>>, BlockingError> {
    let conn = r.sys.process(pid).connect("alfa-broker-t", 10).unwrap();
    let server_side = listener.accept().unwrap();
    let svc = r.service.clone();
    std::thread::spawn(move || svc.handle_connection(server_side));
    BlockingClient::connect(conn, &h)
}

fn listener(r: &Rig) -> Box<dyn platform_contract::PipeListener> {
    r.sys.listen(&config().pipe_security().unwrap()).unwrap()
}

#[test]
fn roles_are_bound_to_verified_process_identity() {
    let r = rig();
    let mut l = listener(&r);
    let mut core = session(&r, &mut l, 10, hello(ClientRole::Core, None, 10)).unwrap();
    assert!(matches!(
        core.call(Request::Metrics).unwrap(),
        Response::Metrics(_)
    ));
    let mut wd = session(&r, &mut l, 15, hello(ClientRole::Watchdog, None, 15)).unwrap();
    assert!(matches!(
        wd.call(Request::KillAll {
            reason: watchdog_contract::KillReason::Hotkey
        })
        .unwrap(),
        Response::Killed(_)
    ));
    // Obcy obraz udający jądro.
    let err = session(&r, &mut l, 11, hello(ClientRole::Core, None, 10))
        .err()
        .unwrap();
    assert!(matches!(err, BlockingError::Rejected(m) if m.contains("obraz")));
    // Rola wyłączona (agentka) i Broker-UI bez poświadczenia (zapis zabroniony).
    assert!(session(&r, &mut l, 10, hello(ClientRole::Agent, None, 10)).is_err());
    let err = session(&r, &mut l, 13, hello(ClientRole::BrokerUi, None, 13))
        .err()
        .unwrap();
    assert!(matches!(err, BlockingError::Rejected(m) if m.contains("wymaga poświadczenia")));
    // Inne konto — odmowa już na DACL potoku.
    assert!(r.sys.process(14).connect("alfa-broker-t", 10).is_err());
    let rejected = r
        .audit
        .names()
        .iter()
        .filter(|n| *n == "broker.ipc.rejected")
        .count();
    assert_eq!(rejected, 3);
}

#[test]
fn broker_ui_needs_ticket_high_integrity_and_its_image() {
    let r = rig();
    let mut sup = UiSupervisor::new(config().broker_ui.as_ref().unwrap());
    let base = UiLaunchTicket {
        credential: r
            .engine
            .issue_client_credential("x", ClientRole::BrokerUi, 1),
        pipe: "alfa-broker-t".into(),
        broker_user: Some(BROKER.into()),
    };
    let ui_cfg = config().broker_ui.unwrap();
    let step = sup.step(
        &r.engine,
        r.launcher.as_ref(),
        &ui_cfg,
        &base,
        r.clock.now_ms(),
    );
    assert!(step.error.is_none());
    let (pid, spec) = r.launcher.launched().pop().unwrap();
    assert_eq!(sup.pid(), Some(pid));
    assert_eq!(spec.integrity, LaunchIntegrity::UserSessionHigh);
    assert!(
        spec.args.is_empty(),
        "poświadczenie nigdy w wierszu poleceń"
    );
    let ticket: UiLaunchTicket = serde_json::from_slice(&spec.stdin).unwrap();
    assert_eq!(ticket.broker_user.as_deref(), Some(BROKER));
    let cred = ticket.credential;
    let mut l = listener(&r);
    let err = session(
        &r,
        &mut l,
        12,
        hello(ClientRole::BrokerUi, Some(cred.clone()), 12),
    );
    assert!(matches!(err, Err(BlockingError::Rejected(m)) if m.contains("integralności")));
    let mut ui = session(
        &r,
        &mut l,
        13,
        hello(ClientRole::BrokerUi, Some(cred.clone()), 13),
    )
    .unwrap();
    assert!(matches!(
        ui.call(Request::PendingApprovals).unwrap(),
        Response::Pending(_)
    ));
    // Poświadczenie Broker-UI w obcym obrazie jądra nic nie daje.
    let mut stolen = cred;
    stolen.role = ClientRole::Core;
    assert!(session(&r, &mut l, 10, hello(ClientRole::Core, Some(stolen), 10)).is_err());

    // Nadzór: proces padł → przerwa → nowe uruchomienie z nowym poświadczeniem.
    r.launcher.exit(pid);
    let wait = sup.step(
        &r.engine,
        r.launcher.as_ref(),
        &ui_cfg,
        &base,
        r.clock.now_ms(),
    );
    assert_eq!(wait.wait_ms, 1_000);
    sup.step(
        &r.engine,
        r.launcher.as_ref(),
        &ui_cfg,
        &base,
        r.clock.now_ms(),
    );
    assert_eq!(sup.launches(), 2);
    let all = r.launcher.launched();
    assert_ne!(all[0].1.stdin, all[1].1.stdin);
    r.launcher.exit(sup.pid().unwrap());
    sup.step(
        &r.engine,
        r.launcher.as_ref(),
        &ui_cfg,
        &base,
        r.clock.now_ms(),
    );
    r.launcher.fail_with(Some("brak SeTcbPrivilege"));
    let failed = sup.step(
        &r.engine,
        r.launcher.as_ref(),
        &ui_cfg,
        &base,
        r.clock.now_ms(),
    );
    assert!(failed.error.unwrap().contains("SeTcbPrivilege"));
    assert_eq!(failed.wait_ms, 4_000, "przerwa rośnie");
}

#[test]
fn start_fails_on_squatted_pipe_and_serves_when_free() {
    let r = rig();
    let squat = PipeSecurity::new("alfa-broker-t", sid(USER), vec![sid(USER)]).unwrap();
    let squatter = r.sys.process(10).listen(&squat).unwrap();
    let err = r.service.start().err().unwrap();
    assert!(err.contains("przejęcie nazwy"), "{err}");
    drop(squatter);
    let threads = r.service.start().unwrap();
    assert_eq!(threads.len(), 2, "przyjmowanie + nadzór Broker-UI");
    let conn = r.sys.process(10).connect("alfa-broker-t", 10).unwrap();
    let mut core = BlockingClient::connect(conn, &hello(ClientRole::Core, None, 10)).unwrap();
    assert!(matches!(
        core.call(Request::Metrics).unwrap(),
        Response::Metrics(_)
    ));
}

#[test]
fn config_validation_and_private_audit_dir() {
    let mut c = config();
    assert!(c.validate().is_ok());
    c.bindings.broker_ui.as_mut().unwrap().enroll = true;
    assert!(c.validate().unwrap_err().contains("zabroniony"));
    let mut c = config();
    c.bindings
        .broker_ui
        .as_mut()
        .unwrap()
        .requirement
        .min_integrity = IntegrityLevel::Medium;
    assert!(c.validate().unwrap_err().contains("UIPI"));
    c.dev_mode = true;
    assert!(
        c.validate().is_ok(),
        "tryb deweloperski: średnia integralność dozwolona"
    );
    let mut c = config();
    c.bindings.core.as_mut().unwrap().requirement.images.clear();
    assert!(c.validate().is_err());
    let mut c = config();
    c.pipe_name = r"zła\nazwa".into();
    assert!(c.validate().is_err());
    let json = serde_json::to_string(&config()).unwrap();
    assert_eq!(
        serde_json::from_str::<ServiceConfig>(&json).unwrap(),
        config()
    );

    let dir = tempfile::tempdir().unwrap();
    let dirs = FakePrivateDirs::default();
    let clock: Arc<dyn Clock> = Arc::new(ManualClock::new(5));
    let data = dir.path().join("broker");
    let writer = open_audit(&dirs, &data, &sid(BROKER), clock.clone(), Some("pre")).unwrap();
    assert_eq!(writer.verify_chain().unwrap().records, 1);
    assert_eq!(dirs.ensured(), vec![(data.clone(), sid(BROKER))]);
    assert!(data.join("audit-anchor.json").exists());
    drop(writer);
    let again = open_audit(&dirs, &data, &sid(BROKER), clock, None).unwrap();
    assert_eq!(
        again.verify_chain().unwrap().records,
        1,
        "kotwica zgodna po ponownym otwarciu"
    );
}
