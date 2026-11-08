//! Instalacja globalna (osobna binarka testu = osobny proces): zdarzenia i panika trafiają do
//! pliku (z redakcją), druga instalacja jest odrzucana.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_logs::{LogConfig, install, list_files};

#[test]
fn install_writes_events_and_panics_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = LogConfig::for_process("alfa-test", Some(dir.path().to_path_buf()));
    config.filter = "info".into();
    config.filter_from_env = false;
    config.stderr = false;
    let handle = install(config.clone()).unwrap();
    assert!(install(config).is_err(), "drugi subskrybent odrzucony");

    tracing::info!(target: "app_core::test", "po instalacji");
    let crashed = std::thread::spawn(|| {
        panic!("awaria testowa z kluczem sk-ant-api03-abcdefghijklmnop");
    })
    .join();
    assert!(crashed.is_err());

    let files = list_files(dir.path(), "alfa-test").unwrap();
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0].path).unwrap();
    assert!(text.contains("app_core::test: po instalacji"), "{text}");
    assert!(
        text.contains("ERROR alfa_panic: panika: awaria testowa"),
        "{text}"
    );
    assert!(text.contains("miejsce="), "{text}");
    assert!(!text.contains("abcdefghijklmnop"), "{text}");
    assert_eq!(handle.current_file().unwrap(), files[0].path);
    assert_eq!(handle.failed_writes(), 0);
}
