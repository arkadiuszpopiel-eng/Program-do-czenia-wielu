//! Atrapy portów Jądra: potok z ACL (inne konto, niska integralność, przejęcie nazwy), tożsamość
//! procesu, okno zatwierdzeń, uruchamianie, katalogi prywatne, host usługi, MMCSS, dysk.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use platform_contract::{
    ApprovalSurfacePort, DiskPort, DiskSpace, InputDevice, InputSample, IntegrityLevel,
    LaunchIntegrity, MmcssPort, MmcssTask, PeerIdentity, PipeSecurity, PlatformError,
    PrivateDirPort, ProcessIdentityPort, SecurePipePort, ServiceHostPort, SessionLaunch,
    SessionLauncherPort, Sid, SignatureStatus, SurfaceButton, SurfaceEvent, SurfaceTone,
    SurfaceView,
};
use platform_fake::{
    FakeDisk, FakeLauncher, FakeMmcss, FakePipes, FakePrivateDirs, FakeServiceHost, FakeSurface,
};

fn id(pid: u32, user: &str, integrity: IntegrityLevel) -> PeerIdentity {
    PeerIdentity {
        pid,
        image: PathBuf::from(format!(r"C:\Alfa\p{pid}.exe")),
        user: Sid::parse(user).unwrap(),
        integrity,
        session: 1,
        signature: SignatureStatus::NotVerified,
    }
}

const BROKER: &str = "S-1-5-80-1-2-3-4-5";
const USER: &str = "S-1-5-21-1-2-3-1001";
const OTHER: &str = "S-1-5-21-1-2-3-1002";

#[test]
fn pipe_acl_identity_and_round_trip() {
    let server = FakePipes::new(id(1, BROKER, IntegrityLevel::System));
    server.register(id(2, USER, IntegrityLevel::Medium));
    server.register(id(3, OTHER, IntegrityLevel::Medium));
    server.register(id(4, USER, IntegrityLevel::Low));
    let sec = PipeSecurity::new(
        "alfa-test",
        Sid::parse(BROKER).unwrap(),
        vec![Sid::parse(USER).unwrap()],
    )
    .unwrap();
    let mut listener = server.listen(&sec).unwrap();
    assert!(
        server.process(9).listen(&sec).is_err(),
        "nieznany proces / zajęta nazwa"
    );
    let squat = server.process(2).listen(&sec);
    assert!(matches!(squat, Err(PlatformError::PermissionDenied(_))));
    assert!(matches!(
        server.process(3).connect("alfa-test", 10),
        Err(PlatformError::PermissionDenied(m)) if m.contains("DACL")
    ));
    assert!(matches!(
        server.process(4).connect("alfa-test", 10),
        Err(PlatformError::PermissionDenied(m)) if m.contains("etykieta")
    ));
    assert_eq!(server.rejected().len(), 2);
    assert!(matches!(
        server.process(2).connect("brak", 10),
        Err(PlatformError::NotFound(_))
    ));
    let mut client = server.process(2).connect("alfa-test", 10).unwrap();
    assert_eq!(client.peer_pid(), 1, "klient widzi PID serwera");
    let mut conn = listener.accept().unwrap();
    assert_eq!(conn.peer_pid(), 2, "serwer widzi PID klienta");
    client.write_all(b"ping").unwrap();
    client.flush().unwrap();
    let mut buf = [0u8; 4];
    conn.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"ping");
    conn.write_all(b"ok").unwrap();
    let mut two = [0u8; 2];
    client.read_exact(&mut two).unwrap();
    assert_eq!(&two, b"ok");
    drop(client);
    let mut rest = Vec::new();
    assert_eq!(conn.read_to_end(&mut rest).unwrap(), 0, "koniec strumienia");
    assert!(conn.write_all(b"x").is_err());
    assert_eq!(server.identify(2).unwrap().user.as_str(), USER);
    assert!(server.identify(77).is_err());
    assert_eq!(server.process(3).current_user().unwrap().as_str(), OTHER);
    drop(listener);
    assert!(server.listen(&sec).is_ok(), "po zamknięciu nazwa wolna");
}

fn view() -> SurfaceView {
    SurfaceView {
        title: "Prośba".into(),
        badge: "⚠ Ryzyko: wysokie".into(),
        tone: SurfaceTone::High,
        details: vec![],
        status: String::new(),
        buttons: vec![SurfaceButton {
            id: 100,
            label: "Odmów".into(),
        }],
        initial_focus: 100,
        take_focus: false,
    }
}

#[test]
fn surface_records_views_and_replays_events() {
    let s = FakeSurface::new();
    assert!(s.current().is_none());
    let mut bad = view();
    bad.initial_focus = 5;
    assert!(s.present(&bad).is_err());
    s.present(&view()).unwrap();
    assert_eq!(s.current().unwrap().title, "Prośba");
    assert_eq!(s.next_event(0), None);
    let input = InputSample {
        device: InputDevice::Mouse,
        injected: false,
        at_ms: 5,
    };
    s.push(SurfaceEvent::Activated { at_ms: 1 });
    s.push(SurfaceEvent::Cancel { input });
    assert_eq!(s.next_event(0), Some(SurfaceEvent::Activated { at_ms: 1 }));
    assert_eq!(s.next_event(10), Some(SurfaceEvent::Cancel { input }));
    s.dismiss().unwrap();
    s.dismiss().unwrap();
    assert_eq!(s.dismissals(), 1);
    assert_eq!(s.presented().len(), 1);
}

#[test]
fn launcher_dirs_service_mmcss_disk() {
    let l = FakeLauncher::new();
    let spec = SessionLaunch {
        image: PathBuf::from(r"C:\Alfa\alfa-broker-ui.exe"),
        args: vec![],
        integrity: LaunchIntegrity::UserSessionHigh,
        stdin: b"{}".to_vec(),
    };
    let pid = l.launch(&spec).unwrap();
    assert!(l.is_running(pid));
    l.exit(pid);
    assert!(!l.is_running(pid));
    l.fail_with(Some("brak SeTcbPrivilege"));
    assert!(l.launch(&spec).is_err());
    l.fail_with(None);
    assert_eq!(l.launched().len(), 1);

    let dir = std::env::temp_dir().join(format!("alfa-fake-priv-{}", std::process::id()));
    let dirs = FakePrivateDirs::default();
    dirs.ensure_private_dir(&dir, &Sid::parse(BROKER).unwrap())
        .unwrap();
    assert!(dir.is_dir());
    assert_eq!(dirs.ensured().len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();

    let host = FakeServiceHost::default();
    host.stop_signal().stop();
    host.run_service(
        "AlfaBroker",
        Box::new(|stop| {
            assert!(stop.is_stopped());
            Ok(())
        }),
    )
    .unwrap();
    assert!(
        host.run_service("x", Box::new(|_| Err("awaria".into())))
            .is_err()
    );
    assert_eq!(host.names(), ["AlfaBroker", "x"]);

    let m = FakeMmcss::default();
    {
        let b = m.boost_current_thread(MmcssTask::ProAudio).unwrap();
        assert_eq!(b.task(), MmcssTask::ProAudio);
        assert_eq!(m.counts(), (1, 0));
    }
    assert_eq!(m.counts(), (1, 1));

    let d = FakeDisk::default();
    let small = DiskSpace {
        available_bytes: 1,
        total_bytes: 10,
        free_bytes: 2,
    };
    d.set(r"C:\", small);
    assert_eq!(d.free_disk_space(Path::new(r"C:\")).unwrap(), small);
    assert!(
        d.free_disk_space(Path::new("D:/x"))
            .unwrap()
            .available_bytes
            > 1
    );
    assert!(d.free_disk_space(Path::new("")).is_err());
}
