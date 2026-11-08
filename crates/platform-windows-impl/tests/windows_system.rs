//! Testy na prawdziwym Windows (CI `windows-latest` i runner self-hosted). Testy wymagające
//! pulpitu użytkownika (skróty globalne, schowek, Kosz z powłoką) są `#[ignore]` —
//! uruchamia je self-hosted runner: `cargo test -p platform-windows-impl -- --ignored`.

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use platform_contract::{
    ClipboardContent, ClipboardPort, FsPort, HardwarePort, Hotkey, HotkeyPort, Integrity, Key,
    KnownFolder, Modifiers, PlatformError, ProcessHandle, ProcessPort, ProcessSpec, ProcessStatus,
    WindowId, WindowPort,
};
use platform_windows_impl::{JobLimits, WindowsPlatform};

fn system32(exe: &str) -> PathBuf {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    PathBuf::from(root).join("System32").join(exe)
}

fn cmd(args: &[&str], integrity: Integrity) -> ProcessSpec {
    ProcessSpec {
        cmd: system32("cmd.exe"),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        cwd: std::env::temp_dir(),
        integrity,
        memory_limit_mb: None,
    }
}

fn wait_exit(p: &WindowsPlatform, h: ProcessHandle) -> ProcessStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = p.status(h).unwrap();
        if status != ProcessStatus::Running || Instant::now() > deadline {
            return status;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn job_object_kills_whole_tree_quickly() {
    let p = WindowsPlatform::default();
    let h = p
        .spawn(cmd(&["/c", "ping -n 30 127.0.0.1 >NUL"], Integrity::Medium))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while p.processes.tree_size(h).unwrap() < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        p.processes.tree_size(h).unwrap() >= 2,
        "cmd + ping w jednym Job Object"
    );
    let started = Instant::now();
    p.kill_tree(h).unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(200),
        "kill_tree trwał {elapsed:?}"
    );
    assert_eq!(p.status(h).unwrap(), ProcessStatus::Killed);
    let deadline = Instant::now() + Duration::from_secs(2);
    while p.processes.tree_size(h).unwrap() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(p.processes.tree_size(h).unwrap(), 0);
    p.processes.release(h).unwrap();
    assert!(matches!(
        p.status(h),
        Err(PlatformError::UnknownResource(_))
    ));
}

#[test]
fn exit_codes_integrity_and_limits() {
    let p = WindowsPlatform::default();
    let h = p.spawn(cmd(&["/c", "exit 7"], Integrity::Medium)).unwrap();
    assert_eq!(wait_exit(&p, h), ProcessStatus::Exited(7));
    let low = p.spawn(cmd(&["/c", "exit 3"], Integrity::Low)).unwrap();
    assert_eq!(wait_exit(&p, low), ProcessStatus::Exited(3));
    let limited = p
        .processes
        .spawn_with_limits(
            cmd(&["/c", "exit 0"], Integrity::Medium),
            JobLimits {
                memory_mb: Some(256),
                cpu_rate_percent: Some(50),
                affinity_mask: Some(1),
            },
        )
        .unwrap();
    assert_eq!(wait_exit(&p, limited), ProcessStatus::Exited(0));
    assert!(matches!(
        p.spawn(cmd(&["/c", "exit 0"], Integrity::AppContainer)),
        Err(PlatformError::Unsupported(_))
    ));
    let own = std::process::id();
    assert!(p.processes.list().unwrap().iter().any(|i| i.pid == own));
    let _ = p.foreground_is_elevated();
}

#[test]
fn hardware_probe_reports_sane_values() {
    let p = WindowsPlatform::default();
    let cpu = p.cpu().unwrap();
    assert!(cpu.physical_cores >= 1 && cpu.logical_cores >= cpu.physical_cores);
    assert!(p.memory_total_mb().unwrap() >= 1024);
    let os = p.os().unwrap();
    assert!(os.name.starts_with("Windows"));
    let guid = p.machine_seed().unwrap().expect("MachineGuid w rejestrze");
    assert_eq!(guid.len(), 36);
    assert!(p.gpus().is_ok());
    assert!(p.npu().is_ok());
    let power = p.power_status().unwrap();
    assert!(power.battery_percent.is_none_or(|pct| pct <= 100));
    // Serwer CI może nie mieć usługi audio — wynik niekoniecznie Ok, ale bez paniki.
    let _ = p.audio_endpoints();
}

#[test]
fn windows_listing_and_unknown_handles() {
    let p = WindowsPlatform::default();
    let details = p.windows.list_detailed();
    let mut ids: Vec<u64> = details.iter().map(|d| d.info.id.0).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), details.len());
    // Okno mogło zniknąć między wyliczeniem a odczytem — wtedy DPI = 0.
    assert!(details.iter().all(|d| d.dpi == 0 || d.dpi >= 96));
    let _ = p.fullscreen_app_active();
    assert!(matches!(
        p.focus(WindowId(0)),
        Err(PlatformError::UnknownResource(_))
    ));
    assert!(p.windows.minimize(WindowId(0x7FFF_FFF0)).is_err());
}

#[test]
fn verbatim_paths_and_junctions_cannot_bypass_denylist() {
    let p = WindowsPlatform::default();
    let dir = tempfile::tempdir().unwrap();
    let secret = dir.path().join(".codex");
    std::fs::create_dir(&secret).unwrap();
    std::fs::write(secret.join("auth.json"), b"token").unwrap();
    let verbatim = PathBuf::from(format!(r"\\?\{}", secret.join("auth.json").display()));
    assert!(matches!(
        p.read(&verbatim),
        Err(PlatformError::Denylisted(_))
    ));
    let junction = dir.path().join("zwykly");
    let status = std::process::Command::new(system32("cmd.exe"))
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&secret)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(matches!(
        p.read(&junction.join("auth.json")),
        Err(PlatformError::Denylisted(_))
    ));
    let local = p.known_folder(KnownFolder::LocalAppData);
    assert!(local.ends_with(r"AppData\Local"), "{}", local.display());
}

#[test]
#[ignore = "wymaga sesji interaktywnej (Kosz powłoki; przy braku Kosza powłoka pyta użytkownika)"]
fn recycle_bin_round_trip() {
    let p = WindowsPlatform::default();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("do-kosza.txt");
    std::fs::write(&file, b"tresc").unwrap();
    let receipt = p.delete_to_recycle_bin(&file).unwrap();
    assert!(receipt.reversible, "wolumin testowy ma Kosz");
    assert!(!file.exists());
    p.undo(receipt.undo.unwrap()).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"tresc");
}

fn test_hotkey(key: u8) -> Hotkey {
    Hotkey::new(
        Modifiers {
            ctrl: true,
            alt: true,
            shift: true,
            win: false,
        },
        Key::Function(key),
    )
}

#[test]
#[ignore = "wymaga sesji interaktywnej (RegisterHotKey na pulpicie użytkownika)"]
fn hotkeys_register_detect_conflicts_and_unregister() {
    let a = WindowsPlatform::default();
    let b = WindowsPlatform::default();
    let id = a.register(test_hotkey(9)).unwrap();
    assert!(matches!(
        a.register(test_hotkey(9)),
        Err(PlatformError::HotkeyRejected(_))
    ));
    assert!(matches!(
        b.register(test_hotkey(9)),
        Err(PlatformError::HotkeyConflict(_))
    ));
    a.unregister(id).unwrap();
    let again = b.register(test_hotkey(9)).unwrap();
    b.unregister(again).unwrap();
    let kill = a.hotkeys.register_kill_switch().unwrap();
    a.unregister(kill).unwrap();
    assert!(a.drain_events().is_empty());
}

#[test]
#[ignore = "wymaga sesji interaktywnej (schowek pulpitu użytkownika)"]
fn clipboard_round_trip_and_privacy() {
    let p = WindowsPlatform::default();
    p.set(ClipboardContent::Text("zażółć gęślą jaźń".into()))
        .unwrap();
    assert_eq!(
        p.get().unwrap(),
        ClipboardContent::Text("zażółć gęślą jaźń".into())
    );
    let files = vec![std::env::temp_dir().join("a.txt"), system32("cmd.exe")];
    p.set(ClipboardContent::Files(files.clone())).unwrap();
    assert_eq!(p.get().unwrap(), ClipboardContent::Files(files));
    assert!(p.restore_previous().unwrap());
    assert_eq!(
        p.get().unwrap(),
        ClipboardContent::Text("zażółć gęślą jaźń".into())
    );
    p.clipboard
        .set_sensitive(ClipboardContent::Text("hasło".into()))
        .unwrap();
    assert!(matches!(p.get(), Err(PlatformError::PermissionDenied(_))));
    p.set(ClipboardContent::Empty).unwrap();
    assert_eq!(p.get().unwrap(), ClipboardContent::Empty);
}
