//! Manifest modułu i złożenie portu (każdy OS).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_registry_contract::{Isolation, Lifecycle, ModuleManifest};
use platform_contract::{HardwarePort, SystemPort, TrayPort, TrayState};
use platform_windows_impl::{MODULE_TOML, PlatformConfig, WindowsPlatform};

fn assert_ports<T: SystemPort + HardwarePort>(_: &T) {}

#[test]
fn manifest_is_valid_and_matches_crate() {
    let m = ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "platform-windows");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.lifecycle, Lifecycle::Always);
    assert_eq!(m.isolation, Isolation::InProc);
    assert_eq!(m.provides[0].to_string(), "platform-contract@1");
    assert!(m.budget.ram_mb <= 4);
}

#[test]
fn platform_is_a_full_system_port() {
    let p = WindowsPlatform::new(PlatformConfig::default());
    assert_ports(&p);
    assert_eq!(p.state(), TrayState::Idle);
    p.set_state(TrayState::Listening).unwrap();
    assert_eq!(p.tray.state(), TrayState::Listening);
    let parsed: PlatformConfig =
        serde_json::from_str(r#"{"clipboard":{"exclude_from_history":false}}"#).unwrap();
    assert!(!parsed.clipboard.exclude_from_history);
    assert!(
        !parsed.fs.extra_deny_names.is_empty(),
        "brakujące pola = wartości domyślne"
    );
}

#[test]
fn delegations_reach_sub_ports() {
    use platform_contract::{
        ClipboardContent, ClipboardPort, FsPort, HotkeyId, HotkeyPort, KnownFolder, Notification,
        ProcessHandle, ProcessPort, TrayMenuItem, UndoToken, WindowId, WindowPort,
    };
    let p = WindowsPlatform::default();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    let receipt = p.write_atomic(&file, b"x").unwrap();
    assert!(p.exists(&file));
    assert_eq!(p.read(&file).unwrap(), b"x");
    let copy = p.copy(&file, &dir.path().join("b.txt")).unwrap();
    let moved = p
        .move_path(&dir.path().join("b.txt"), &dir.path().join("c.txt"))
        .unwrap();
    assert_eq!(p.list_dir(dir.path()).unwrap().len(), 2);
    p.undo(moved.undo.unwrap()).unwrap();
    p.undo(copy.undo.unwrap()).unwrap();
    p.undo(receipt.undo.unwrap()).unwrap();
    assert!(p.undo(UndoToken(999)).is_err());
    std::fs::write(&file, b"y").unwrap();
    assert!(!p.delete_permanent(&file).unwrap().reversible);
    assert!(p.known_folder(KnownFolder::Temp).is_absolute());
    assert!(p.status(ProcessHandle(1)).is_err());
    assert!(p.kill_tree(ProcessHandle(1)).is_err());
    assert!(p.unregister(HotkeyId(1)).is_err());
    assert!(p.drain_events().is_empty());
    assert!(p.focus(WindowId(0)).is_err());
    assert!(p.restore_previous().is_ok());
    p.set_menu(vec![TrayMenuItem {
        id: "quit".into(),
        label: "Zakończ".into(),
        enabled: true,
    }])
    .unwrap();
    let toast = Notification {
        title: "t".into(),
        body: "b".into(),
    };
    assert!(p.notify(toast).is_err(), "bez backendu Tauri");
    if !cfg!(windows) {
        // Na Windows Kosz testuje `windows_system.rs` (#[ignore], powłoka może zapytać użytkownika).
        assert!(p.delete_to_recycle_bin(dir.path()).is_err());
        assert!(
            p.spawn(platform_contract::ProcessSpec {
                cmd: "/bin/true".into(),
                args: vec![],
                cwd: "/".into(),
                integrity: Default::default(),
                memory_limit_mb: None,
            })
            .is_err()
        );
        assert!(!p.foreground_is_elevated());
        assert!(p.get().is_err() && p.set(ClipboardContent::Empty).is_err());
        assert!(p.list().is_empty());
        let key = platform_contract::Hotkey::new(
            platform_contract::Modifiers {
                ctrl: true,
                ..Default::default()
            },
            platform_contract::Key::Function(9),
        );
        assert!(p.register(key).is_err());
        assert!(p.os().is_err() && p.cpu().is_err() && p.memory_total_mb().is_err());
        assert!(p.gpus().is_err() && p.npu().is_err() && p.power_status().is_err());
        assert!(p.audio_endpoints().is_err() && p.machine_seed().is_err());
    }
}
