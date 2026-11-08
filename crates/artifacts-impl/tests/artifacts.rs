//! Kontrakt współdzielony na bazach SQLCipher + wersje niezmienne, szyfrowanie migawek, zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};

use artifacts_contract::contract_tests;
use artifacts_contract::{ArtifactError, Artifacts, Origin, Preview, SessionId, events};
use artifacts_impl::SqliteArtifacts;
use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use sessions_contract::SessionDbProvider;
use sessions_fake::TempDbProvider;

struct Harness {
    provider: Arc<TempDbProvider>,
    artifacts: SqliteArtifacts,
}

impl Deref for Harness {
    type Target = SqliteArtifacts;

    fn deref(&self) -> &SqliteArtifacts {
        &self.artifacts
    }
}

fn harness() -> Harness {
    let provider = Arc::new(TempDbProvider::new().unwrap());
    let artifacts = SqliteArtifacts::new(provider.clone(), "C:/Users/ja/Alfa".into()).unwrap();
    Harness {
        provider,
        artifacts,
    }
}

#[test]
fn contract_suite() {
    contract_tests::run_all(harness);
}

#[test]
fn versions_are_immutable_and_snapshots_encrypted() {
    let h = harness();
    let dir = tempfile::tempdir().unwrap();
    let secret = "artefakt-szpieg-7070";
    let path = dir.path().join("tajne.txt");
    std::fs::write(&path, secret).unwrap();
    let s = SessionId::new("S");
    h.register(&s, &path, Origin::User, None).unwrap();
    let db = h.provider.session_db(&s).unwrap();
    db.with(|c| {
        for sql in [
            "UPDATE artifact_versions SET meta = '{}'",
            "DELETE FROM artifact_versions",
        ] {
            let err = c.execute(sql, []).unwrap_err().to_string();
            assert!(err.contains("niezmienne"), "{sql}: {err}");
        }
        Ok::<(), ()>(())
    })
    .unwrap();
    for file in lib_sqlstore::database_files(db.path()) {
        let bytes = std::fs::read(&file).unwrap_or_default();
        assert!(!bytes.windows(secret.len()).any(|w| w == secret.as_bytes()));
    }
}

#[test]
fn large_files_have_no_snapshot_and_preview_is_fast() {
    let h = harness();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("duzy.log");
    let line = "2026-09-30 wiersz dziennika z polskimi znakami: żółć\n";
    std::fs::write(&path, line.repeat(40_000)).unwrap();
    let s = SessionId::new("S");
    let art = h.register(&s, &path, Origin::User, None).unwrap();
    assert!(
        !art.versions[0].snapshot,
        "{} B > 1 MiB",
        art.versions[0].bytes
    );
    let t = Instant::now();
    let preview = h.preview(&s, &art.id, None, 1024 * 1024).unwrap();
    let elapsed = t.elapsed();
    assert!(matches!(
        preview,
        Preview::Text {
            truncated: true,
            ..
        }
    ));
    eprintln!("[budżet artifacts] podgląd 1 MiB: {elapsed:?}");
    assert!(
        elapsed.as_millis() < budget_ms(100),
        "podgląd 1 MiB: {elapsed:?}"
    );
    std::fs::write(&path, "nadpisany").unwrap();
    assert!(matches!(
        h.preview(&s, &art.id, Some(1), 100),
        Err(ArtifactError::ContentUnavailable { .. })
    ));
}

#[tokio::test]
async fn module_events() {
    let mut h = harness();
    assert_eq!(h.artifacts.health(), HealthStatus::NotStarted);
    assert_eq!(h.artifacts.manifest().id.as_str(), "artifacts");
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(h.artifacts.manifest().id.clone(), Arc::new(bus.clone()));
    h.artifacts.start(ctx).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.txt");
    std::fs::write(&path, "1").unwrap();
    let s = SessionId::new("S");
    let art = h.register(&s, &path, Origin::User, None).unwrap();
    std::fs::write(&path, "2").unwrap();
    h.register(&s, &path, Origin::User, None).unwrap();
    h.intent(
        &s,
        &art.id,
        None,
        artifacts_contract::ArtifactAction::Reveal,
    )
    .unwrap();
    let kind = EventKind::Custom(events::EXPORTED.to_owned());
    for _ in 0..200 {
        if !bus.recorded_of_kind(&kind).is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let count = |k: &str| bus.recorded_of_kind(&EventKind::Custom(k.to_owned())).len();
    assert_eq!(
        (
            count(events::REGISTERED),
            count(events::VERSION_ADDED),
            count(events::EXPORTED)
        ),
        (1, 1, 1)
    );
    h.artifacts.stop().await.unwrap();
}

/// Budżet czasowy: ściśle przy `ALFA_PERF_BUDGETS=1` (maszyna pomiarowa, baseline),
/// na współdzielonym CI tylko próg bezpieczeństwa ×10 (łapie patologiczne regresje).
fn budget_ms(strict_ms: u128) -> u128 {
    if std::env::var_os("ALFA_PERF_BUDGETS").is_some() {
        strict_ms
    } else {
        strict_ms * 10
    }
}
