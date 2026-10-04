//! Pełny łańcuch procesów Jądra na atrapach platformy (każdy system): usługa Brokera na potoku
//! z ACL → bilet Broker-UI na stdin → okno (FakeSurface) → decyzja z dowodem przez potok →
//! token dla jądra; wstrzyknięte kliknięcie i clickjacking = 0 sukcesów; kill-switch watchdoga
//! przez potok (rola `Watchdog` po tożsamości obrazu) unieważnia tokeny.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use app_safety::broker::{build_engine, dev_config};
use app_safety::watchdog::{WatchdogArgs, broker_peer};
use broker_ui_contract::{BrokerUi, NoHello, UiConfig};
use broker_ui_impl::driver::cycle;
use broker_ui_impl::{BTN_ONCE, NativeBrokerUi, PipeLink};
use platform_contract::{
    InputDevice, InputSample, IntegrityLevel, LaunchIntegrity, PeerIdentity, SecurePipePort, Sid,
    SignatureStatus, StopSignal, SurfaceEvent, UnverifiedSignatures,
};
use platform_fake::{FakeLauncher, FakePipes, FakePrivateDirs, FakeProcesses, FakeSurface};
use safety_broker_contract::contract_tests::{delta, host, request, test_policy};
use safety_broker_contract::ipc::{ClientCredential, ClientRole, Hello, Request, Response};
use safety_broker_contract::ipc_blocking::{BlockingClient, UiLaunchTicket};
use safety_broker_contract::{ApprovalStatus, Capability, CommandOrigin, Decision};
use safety_broker_impl::service::{BrokerService, ServicePorts};
use watchdog_contract::{Clock, KillReason, KillSwitch, ManualClock};

const BROKER: &str = "S-1-5-80-9-9-9-9-9";
const USER: &str = "S-1-5-21-5-6-7-1001";
const DIR: &str = r"C:\Program Files\Alfa";

fn ident(pid: u32, user: &str, image: &str, integrity: IntegrityLevel) -> PeerIdentity {
    PeerIdentity {
        pid,
        image: Path::new(DIR).join(image),
        user: Sid::parse(user).unwrap(),
        integrity,
        session: 1,
        signature: SignatureStatus::NotVerified,
    }
}

struct Chain {
    sys: FakePipes,
    clock: Arc<ManualClock>,
    launcher: Arc<FakeLauncher>,
    _service: Arc<BrokerService>,
    _rt: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

fn chain() -> Chain {
    let sys = FakePipes::new(ident(1, BROKER, "alfa-broker.exe", IntegrityLevel::System));
    sys.register(ident(10, USER, "alfa-desktop.exe", IntegrityLevel::Medium));
    sys.register(ident(13, USER, "alfa-broker-ui.exe", IntegrityLevel::High));
    sys.register(ident(15, USER, "alfa-watchdog.exe", IntegrityLevel::Medium));
    let dir = tempfile::tempdir().unwrap();
    let user = Sid::parse(USER).unwrap();
    let mut config = dev_config(
        Path::new(DIR),
        user,
        r"C:\Users\ala",
        dir.path().join("broker"),
    );
    config.broker_user = Sid::parse(BROKER).unwrap();
    config.dev_mode = false;
    config.pipe_name = "alfa-broker".into();
    if let Some(ui) = config.bindings.broker_ui.as_mut() {
        ui.requirement.min_integrity = IntegrityLevel::High;
    }
    if let Some(ui) = config.broker_ui.as_mut() {
        ui.integrity = LaunchIntegrity::UserSessionHigh;
    }
    let clock = Arc::new(ManualClock::new(1_000_000));
    let processes = Arc::new(FakeProcesses::default());
    let engine = build_engine(
        &config,
        test_policy(),
        &FakePrivateDirs::default(),
        processes,
        clock.clone(),
    )
    .unwrap();
    let launcher = Arc::new(FakeLauncher::new());
    let ports = ServicePorts {
        pipes: Arc::new(sys.clone()),
        identity: Arc::new(sys.clone()),
        signatures: Arc::new(UnverifiedSignatures),
        launcher: launcher.clone(),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = BrokerService::new(
        engine,
        config,
        ports,
        rt.handle().clone(),
        StopSignal::new(),
    )
    .unwrap();
    let service = Arc::new(service);
    service.start().unwrap();
    Chain {
        sys,
        clock,
        launcher,
        _service: service,
        _rt: rt,
        _dir: dir,
    }
}

fn ticket(c: &Chain) -> UiLaunchTicket {
    for _ in 0..200 {
        if let Some((_, spec)) = c.launcher.launched().first() {
            return serde_json::from_slice(&spec.stdin).unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("usługa nie uruchomiła Broker-UI");
}

fn core_client(c: &Chain) -> BlockingClient<Box<dyn platform_contract::PipeConnection>> {
    let conn = c.sys.process(10).connect("alfa-broker", 100).unwrap();
    let hello = Hello {
        protocol: 1,
        credential: ClientCredential {
            client_id: "core".into(),
            role: ClientRole::Core,
            expires_at_ms: 0,
            mac: String::new(),
        },
        pid: 10,
        sid: None,
        image: None,
    };
    BlockingClient::connect(conn, &hello).unwrap()
}

fn click(c: &Chain, surface: &FakeSurface, injected: bool) {
    let input = InputSample {
        device: InputDevice::Mouse,
        injected,
        at_ms: c.clock.now_ms(),
    };
    surface.push(SurfaceEvent::Button {
        id: BTN_ONCE,
        input,
        occluded: false,
    });
}

#[test]
fn approval_crosses_all_processes_and_injection_never_succeeds() {
    let c = chain();
    let t = ticket(&c);
    assert_eq!(t.broker_user.as_deref(), Some(BROKER));
    let ui_proc = c.sys.process(13);
    let mut link = PipeLink::connect(&ui_proc, &ui_proc, &t, 13).unwrap();
    let surface = Arc::new(FakeSurface::new());
    let mut ui = NativeBrokerUi::new(surface.clone(), Arc::new(NoHello), UiConfig::default());

    let mut core = core_client(&c);
    let egress = request(
        &delta(),
        Capability::NetEgress(host("x.example.org")),
        CommandOrigin::UserText,
    );
    let Response::Decision(Decision::NeedsApproval(ticket)) =
        core.call(Request::Decide(egress)).unwrap()
    else {
        panic!("oczekiwano prośby o zatwierdzenie")
    };
    let now = c.clock.now_ms();
    cycle(&mut ui, &mut link, now, 0, true).unwrap();
    assert_eq!(ui.queued(), vec![ticket.id]);
    surface.push(SurfaceEvent::Activated { at_ms: now });
    c.clock.advance(100);
    click(&c, &surface, false);
    assert_eq!(
        cycle(&mut ui, &mut link, c.clock.now_ms(), 0, false)
            .unwrap()
            .resolved,
        None,
        "clickjacking"
    );
    c.clock.advance(1_000);
    for _ in 0..100 {
        click(&c, &surface, true);
        let r = cycle(&mut ui, &mut link, c.clock.now_ms(), 0, false).unwrap();
        assert_eq!(r.resolved, None, "SendInput nigdy nie rozstrzyga prośby");
    }
    click(&c, &surface, false);
    let r = cycle(&mut ui, &mut link, c.clock.now_ms(), 0, false).unwrap();
    assert_eq!(r.resolved, Some(ticket.id));
    let status = core
        .call(Request::ApprovalStatus {
            id: ticket.id,
            requester: delta(),
        })
        .unwrap();
    assert!(matches!(
        status,
        Response::Status(ApprovalStatus::Approved { token: Some(_) })
    ));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    // Tryb przenośny: serwer potoku musi być procesem Brokera uruchomionym przez aplikację.
    let squatter = app_safety::watchdog::broker_peer_checked(
        Arc::new(c.sys.process(15)),
        Arc::new(c.sys.process(15)),
        "alfa-broker".into(),
        None,
        Some(999),
    );
    let report = rt.block_on(squatter.kill_all(KillReason::Hotkey));
    assert_eq!(report.tokens_revoked, 0, "zły PID serwera — bez wywołania");
    let peer = app_safety::watchdog::broker_peer_checked(
        Arc::new(c.sys.process(15)),
        Arc::new(c.sys.process(15)),
        "alfa-broker".into(),
        Some(Sid::parse(BROKER).unwrap()),
        Some(1),
    );
    let report = rt.block_on(peer.kill_all(KillReason::Hotkey));
    assert!(report.tokens_revoked >= 1, "{report:?}");
    let impostor = broker_peer(
        Arc::new(c.sys.process(15)),
        Arc::new(c.sys.process(15)),
        "alfa-broker".into(),
        Some(Sid::parse(USER).unwrap()),
    );
    let report = rt.block_on(impostor.kill_all(KillReason::Hotkey));
    assert_eq!(
        report.tokens_revoked, 0,
        "zła tożsamość serwera — bez wywołania"
    );
}

#[test]
fn watchdog_args_and_child_launcher() {
    let a: Vec<String> = [
        "--broker-pipe",
        "p1",
        "--broker-user",
        BROKER,
        "--",
        r"C:\a\core.exe",
        "-x",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let w = WatchdogArgs::parse(&a).unwrap();
    assert_eq!(w.broker_pipe, "p1");
    assert_eq!(w.broker_user.unwrap().as_str(), BROKER);
    assert_eq!(
        w.core,
        Some((PathBuf::from(r"C:\a\core.exe"), vec!["-x".to_owned()]))
    );
    assert_eq!(
        WatchdogArgs::parse(&[]).unwrap().broker_pipe,
        "alfa-broker-dev"
    );
    assert!(WatchdogArgs::parse(&["--broker-user".into(), "zły".into()]).is_err());
    let l = app_safety::ChildLauncher::default();
    use platform_contract::{SessionLaunch, SessionLauncherPort};
    let spec = SessionLaunch {
        image: PathBuf::from("relatywna"),
        args: vec![],
        integrity: LaunchIntegrity::AsCaller,
        stdin: vec![],
    };
    assert!(l.launch(&spec).is_err());
    let high = SessionLaunch {
        image: std::env::current_exe().unwrap(),
        integrity: LaunchIntegrity::UserSessionHigh,
        ..spec
    };
    assert!(
        l.launch(&high).is_err(),
        "ChildLauncher nie udaje wysokiej integralności"
    );
    assert!(!l.is_running(1));
    let mut input = std::io::Cursor::new(b"\n".to_vec());
    assert!(app_safety::ui::read_ticket(&mut input).is_err());
}
