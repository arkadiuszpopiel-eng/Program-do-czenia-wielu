//! Przegląd bezpieczeństwa #3 (2026-10, SR3-01): lista procesów Jądra w polityce Brokera
//! (`PROTECTED_PROCESSES`) musi obejmować **każdy** obraz chroniony przez strażnika celów
//! platformy (`platform_contract::PROTECTED_IMAGES`) — w szczególności właściwy plik aplikacji
//! `alfa-desktop.exe` (`updater_contract::APP_EXE`), `alfa-updater.exe` i `alfa-mcp-proxy.exe`.
//! Inaczej Broker nie stosuje twardej blokady `gui.control` wobec okna Alfy, a leksykalny
//! strażnik powłoki przepuszcza `taskkill /im alfa-desktop.exe` (pętla awarii → wycofanie
//! wersji przez watchdoga/launcher, zerwanie Brokera trybu przenośnego).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use safety_broker_contract::{
    AppSelector, Capability, DeclaredFacts, KernelGuard, KernelPolicy, KernelRule as K, PathScope,
    ShellContext, check_command,
};

fn policy() -> KernelPolicy {
    KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap()
}

fn check(cmd: &str) -> Option<K> {
    let env = PathEnv::windows_profile(r"C:\Users\ala")
        .with("SystemRoot", r"C:\Windows")
        .with("windir", r"C:\Windows");
    let deny = DenyChecker::new(DenyLists::baseline(), &env);
    let kernel = [PathScope::tree(r"C:\ProgramData\AlfaBroker", &env).unwrap()];
    let ctx = ShellContext {
        env: &env,
        deny: &deny,
        system_drive: 'c',
        kernel_paths: &kernel,
        cwd: None,
    };
    check_command(cmd, &ctx)
}

#[test]
fn every_platform_protected_image_is_a_broker_kernel_process() {
    let policy = policy();
    for image in platform_contract::PROTECTED_IMAGES {
        let app = AppSelector::parse(image).unwrap();
        assert!(
            policy.is_protected_process(&app),
            "{image}: chroniony przez strażnika celów platformy, a nie przez Brokera"
        );
    }
}

#[test]
fn gui_control_of_the_desktop_app_is_a_kernel_block_on_every_level() {
    let guard = KernelGuard::new(policy(), PathEnv::windows_profile(r"C:\Users\ala"));
    for image in [
        "alfa-desktop.exe",
        r"C:\Users\ala\AppData\Local\Alfa\versions\0.3.1\ALFA-DESKTOP.EXE",
        "alfa-updater.exe",
        "alfa-mcp-proxy.exe",
    ] {
        let cap = Capability::GuiControl(AppSelector::parse(image).unwrap());
        assert_eq!(
            guard.check_request(&cap, &DeclaredFacts::new("gui")),
            Some(K::GuiControlOfKernelProcess),
            "{image}"
        );
    }
}

#[test]
fn killing_the_desktop_app_or_updater_is_blocked_in_the_shell() {
    for cmd in [
        "taskkill /F /IM alfa-desktop.exe",
        "taskkill /im ALFA-DESKTOP.EXE /t",
        "Stop-Process -Name alfa-desktop -Force",
        "spps -Name alfa-desktop",
        "taskkill /f /im alfa-updater.exe",
        "Stop-Process -Name alfa-mcp-proxy",
    ] {
        assert_eq!(check(cmd), Some(K::KillSwitchDisable), "{cmd}");
    }
    // Inne procesy nadal można kończyć.
    assert_eq!(check("taskkill /im notepad.exe"), None);
    assert_eq!(check("Stop-Process -Name alfabet"), None);
}
