//! Porty Jądra z `platform-windows-impl` na prawdziwym Windows (CI `windows-latest`, bez
//! pulpitu): DACL potoku egzekwowany (klient spoza listy SID = odmowa), pierwsza instancja,
//! PID-y obu stron, tożsamość procesu, katalog prywatny, MMCSS, wolne miejsce, host usługi poza
//! SCM, uruchamianie „jak wywołujący”. Uruchomienie z wysoką integralnością — `#[ignore]`.

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use app_safety::ChildLauncher;
use platform_contract::{
    DiskPort, IntegrityLevel, LaunchIntegrity, MmcssPort, MmcssTask, PipeSecurity, PlatformError,
    PrivateDirPort, ProcessIdentityPort, SecurePipePort, ServiceHostPort, SessionLaunch,
    SessionLauncherPort, Sid, same_image,
};
use platform_windows_kernel_impl::{WinKernel, WinSessionLauncher};

fn name(tag: &str) -> String {
    format!("alfa-test-{tag}-{}", std::process::id())
}

#[test]
fn pipe_dacl_round_trip_first_instance_and_peer_pids() {
    let k = WinKernel;
    let me = k.current_user().unwrap();
    let sec = PipeSecurity::new(&name("rt"), me.clone(), vec![me.clone()]).unwrap();
    let mut listener = k.listen(&sec).unwrap();
    assert!(
        matches!(k.listen(&sec), Err(PlatformError::PermissionDenied(_))),
        "druga „pierwsza instancja” = przejęcie nazwy"
    );
    let pipe = sec.name().to_owned();
    let client = std::thread::spawn(move || {
        let mut c = WinKernel.connect(&pipe, 2_000).unwrap();
        assert_eq!(c.peer_pid(), std::process::id());
        c.write_all(b"ping").unwrap();
        let mut buf = [0u8; 2];
        c.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"ok");
    });
    let mut conn = listener.accept().unwrap();
    assert_eq!(conn.peer_pid(), std::process::id());
    let mut buf = [0u8; 4];
    conn.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"ping");
    conn.write_all(b"ok").unwrap();
    client.join().unwrap();
    let mut rest = Vec::new();
    assert_eq!(
        conn.read_to_end(&mut rest).unwrap(),
        0,
        "klient zamknięty = koniec"
    );
    assert!(k.connect("alfa-nie-istnieje-xyz", 10).is_err());
}

#[test]
fn pipe_dacl_rejects_account_outside_the_list() {
    // Właściciel i jedyny klient: LocalSystem — bieżące konto (nawet administrator) nie jest na
    // liście, więc otwarcie potoku kończy się odmową dostępu (DACL egzekwowany przez system).
    let system = Sid::parse(Sid::LOCAL_SYSTEM).unwrap();
    let sec = PipeSecurity::new(&name("deny"), system.clone(), vec![system]).unwrap();
    let _listener = WinKernel.listen(&sec).unwrap();
    match WinKernel.connect(sec.name(), 200) {
        Err(PlatformError::PermissionDenied(_)) => {}
        Err(other) => panic!("oczekiwano odmowy dostępu, jest: {other}"),
        Ok(_) => panic!("konto spoza DACL połączyło się z potokiem"),
    }
}

#[test]
fn identity_private_dir_mmcss_disk_service() {
    let k = WinKernel;
    let me = k.identify(std::process::id()).unwrap();
    assert_eq!(me.user, k.current_user().unwrap());
    assert!(me.integrity >= IntegrityLevel::Medium);
    assert!(
        same_image(&me.image, &std::env::current_exe().unwrap()),
        "{:?}",
        me.image
    );
    let dir = std::env::temp_dir().join(format!("alfa-priv-{}", std::process::id()));
    k.ensure_private_dir(&dir, &me.user).unwrap();
    k.ensure_private_dir(&dir, &me.user).unwrap();
    std::fs::write(dir.join("kotwica.json"), b"{}").unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    match k.boost_current_thread(MmcssTask::ProAudio) {
        Ok(boost) => drop(boost),
        // Windows Server bez usługi MMCSS: błąd z kodem, nigdy panika.
        Err(e) => eprintln!("MMCSS niedostępne na runnerze: {e}"),
    }
    let space = k.free_disk_space(&std::env::temp_dir()).unwrap();
    assert!(space.available_bytes > 0 && space.total_bytes >= space.available_bytes);
    let outside_scm = k.run_service("AlfaTest", Box::new(|_| Ok(())));
    assert!(
        outside_scm.is_err(),
        "poza menedżerem usług dyspozytor odmawia"
    );
}

#[test]
fn child_launcher_passes_stdin() {
    let l = ChildLauncher::default();
    let cmd = std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into());
    let spec = SessionLaunch {
        image: PathBuf::from(cmd),
        args: vec!["/c".into(), "more".into()],
        integrity: LaunchIntegrity::AsCaller,
        stdin: b"bilet\n".to_vec(),
    };
    let pid = l.launch(&spec).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while l.is_running(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!l.is_running(pid));
}

#[test]
#[ignore = "self-hosted: usługa LocalSystem (SeTcbPrivilege) i zalogowany użytkownik"]
fn launch_in_user_session_with_high_integrity() {
    let l = WinSessionLauncher::default();
    let spec = SessionLaunch {
        image: PathBuf::from(r"C:\Windows\System32\notepad.exe"),
        args: vec![],
        integrity: LaunchIntegrity::UserSessionHigh,
        stdin: Vec::new(),
    };
    let pid = l.launch(&spec).unwrap();
    assert_eq!(
        WinKernel.identify(pid).unwrap().integrity,
        IntegrityLevel::High
    );
}
