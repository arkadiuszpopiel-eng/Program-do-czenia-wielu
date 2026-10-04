//! Watchdog → launcher: sygnał rollbacku (`updater_impl::WatchdogSignal`) zamiast `updater: None` —
//! wersja bieżąca z `current.json` instalacji albo wersja pakietu; rollback tylko o jeden krok.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_safety::watchdog::updater_signal_in;

#[test]
fn watchdog_gets_an_updater_signal_for_the_install_root() {
    let dir = tempfile::tempdir().unwrap();
    let signal = updater_signal_in(None, dir.path()).expect("sygnał dla katalogu Alfy");
    assert_eq!(signal.current_version(), env!("CARGO_PKG_VERSION"));
    let refused = signal.request_rollback("0.0.0").unwrap_err();
    assert!(!refused.is_empty(), "bez zainstalowanych wersji — odmowa");
    let exe = dir
        .path()
        .join("versions")
        .join("1.2.3")
        .join("alfa-desktop.exe");
    assert!(updater_signal_in(Some(&exe), dir.path()).is_some());
}
