//! Odporność plików instalacji, wektory minisign z dokumentacji, manifest modułu i zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use semver::Version;
use updater_contract::contract_tests::Harness;
use updater_contract::{Release, Updater, UpdaterError, events};
use updater_impl::{FsUpdater, MODULE_TOML, UpdaterConfig};

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

#[test]
fn corrupt_state_file_falls_back_to_newest_and_writes_are_atomic() {
    let h = common::harness();
    h.install("1.0.0");
    h.install("1.2.0");
    h.updater.switch_to(&v("1.0.0")).unwrap();
    let current = h.updater.layout().current.clone();
    let root = h.updater.layout().root.clone();
    let names: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
    std::fs::write(&current, b"{ uszkodzony").unwrap();
    assert_eq!(h.updater.state().unwrap(), None);
    let c = h.updater.select_launch().unwrap();
    assert_eq!((c.version, c.fallback), (v("1.2.0"), true));
    std::fs::write(&current, br#"{"schema":99,"active":"1.0.0","previous":null,"pending":false,"updated_at":"2026-01-01T00:00:00Z"}"#).unwrap();
    assert_eq!(
        h.updater.state().unwrap(),
        None,
        "nieznany schemat = brak stanu"
    );
    h.updater.switch_to(&v("1.0.0")).unwrap();
    assert_eq!(
        h.updater.state().unwrap().map(|s| s.active),
        Some(v("1.0.0"))
    );
}

#[test]
fn only_canonical_and_consistent_version_dirs_count() {
    let h = common::harness();
    h.install("1.0.0");
    let versions = h.updater.layout().versions.clone();
    std::fs::create_dir_all(versions.join("01.0.0")).unwrap();
    std::fs::create_dir_all(versions.join("staging")).unwrap();
    std::fs::write(versions.join("01.0.0").join("alfa-desktop.exe"), b"MZ").unwrap();
    h.install("1.1.0");
    std::fs::write(
        versions.join("1.1.0").join("version.json"),
        br#"{"version":"9.9.9"}"#,
    )
    .unwrap();
    assert_eq!(h.updater.installed().unwrap(), vec![v("1.0.0")]);
    h.updater.switch_to(&v("1.0.0")).unwrap();
    // Sprzątanie usuwa tylko katalogi wersji (uszkodzona 1.1.0 jest nowsza od aktywnej — zostaje),
    // nigdy obcych katalogów.
    assert!(h.updater.prune(1).unwrap().is_empty());
    assert!(versions.join("staging").is_dir() && versions.join("01.0.0").is_dir());
    h.updater.ensure_layout().unwrap();
    assert!(h.updater.layout().webview_data.is_dir());
}

#[test]
fn minisign_documentation_vectors() {
    // Wektory z dokumentacji `minisign-verify` (podpis „prehashed” pliku `test`).
    let dir = tempfile::tempdir().unwrap();
    let mut config = UpdaterConfig::new(dir.path());
    config.public_key = Some("RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3".into());
    config.require_version_tag = false;
    let u = FsUpdater::new(config.clone()).unwrap();
    let signature = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1556193335\tfile:test\ny/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";
    let package = dir.path().join("test");
    std::fs::write(&package, b"test").unwrap();
    let release = Release {
        version: v("1.0.0"),
        url: "https://repo.example/test".into(),
        sha256: common::sha(b"test"),
        minisign: signature.into(),
        notes: String::new(),
        min_previous: None,
    };
    u.verify_release(&release, &package).unwrap();
    std::fs::write(&package, b"Test").unwrap();
    let tampered = Release {
        sha256: common::sha(b"Test"),
        ..release.clone()
    };
    assert!(matches!(
        u.verify_release(&tampered, &package),
        Err(UpdaterError::SignatureInvalid { .. })
    ));
    // Ten sam podpis bez wiązania wersji jest odrzucany w trybie domyślnym.
    config.require_version_tag = true;
    let strict = FsUpdater::new(config.clone()).unwrap();
    std::fs::write(&package, b"test").unwrap();
    assert!(matches!(
        strict.verify_release(&release, &package),
        Err(UpdaterError::SignatureInvalid { .. })
    ));
    config.public_key = None;
    assert_eq!(
        FsUpdater::new(config)
            .unwrap()
            .verify_release(&release, &package),
        Err(UpdaterError::NoPublicKey)
    );
    assert!(updater_impl::public_key("untrusted comment: minisign public key E7620F1842B4E81F\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3").is_ok());
    assert!(updater_impl::public_key("nie-klucz").is_err());
}

#[test]
fn module_manifest_is_valid() {
    let m = core_registry_contract::ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "updater");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn audit_events() {
    let mut h = common::harness();
    let bus = FakeBus::default();
    assert_eq!(h.updater.health(), HealthStatus::NotStarted);
    let ctx = ModuleContext::new(h.updater.manifest().id.clone(), Arc::new(bus.clone()));
    h.updater.start(ctx).await.unwrap();
    for ver in ["1.0.0", "1.1.0", "1.2.0"] {
        h.install(ver);
    }
    h.updater.switch_to(&v("1.0.0")).unwrap();
    h.updater.switch_to(&v("1.1.0")).unwrap();
    h.updater.rollback().unwrap();
    h.updater.prune(1).unwrap();
    let (bad, path) = h.release("1.3.0", updater_contract::contract_tests::Fixture::OtherKey);
    assert!(h.updater.verify_release(&bad, &path).is_err());
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let count = |k: &str| bus.recorded_of_kind(&EventKind::Custom(k.to_owned())).len();
    assert_eq!(count(events::SWITCHED), 2);
    assert_eq!(count(events::ROLLED_BACK), 1);
    assert_eq!(
        count(events::PRUNED),
        0,
        "1.2.0 jest nowsza od aktywnej — zostaje"
    );
    assert_eq!(count(events::SIGNATURE_INVALID), 1);
    h.updater.stop().await.unwrap();
}
