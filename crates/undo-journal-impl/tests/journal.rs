//! Testy implementacji dziennika: kontrakt na `DirStore`, trwałość (restart), odporność na
//! urwany zapis, uszkodzone pre-image (raport częściowy), limit magazynu, zdarzenia, moduł.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use core_bus_contract::SessionId;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use platform_contract::FsPort;
use platform_fake::FakeFs;
use undo_journal_contract::{
    StepCtx, UndoError, UndoJournal, UndoLimits, contract_tests, event_kind,
};
use undo_journal_impl::{DirStore, UndoService};

fn service(fs: Arc<FakeFs>, dir: &Path, limits: UndoLimits, boot: u64) -> UndoService {
    let clock = Arc::new(|| 5_000u64);
    UndoService::open(fs, dir, limits, clock, boot).unwrap()
}

#[test]
fn contract_suite_on_dir_store() {
    let root = tempfile::tempdir().unwrap();
    let n = AtomicU64::new(0);
    contract_tests::run_all(|limits, clock: Arc<AtomicU64>| {
        let dir = root
            .path()
            .join(n.fetch_add(1, Ordering::SeqCst).to_string());
        let fs = Arc::new(FakeFs::new());
        let c = move || clock.load(Ordering::SeqCst);
        let s = UndoService::open(fs.clone(), dir, limits, Arc::new(c), 1).unwrap();
        (s, fs as Arc<dyn FsPort>)
    });
}

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

#[test]
fn survives_restart_and_torn_last_line() {
    let dir = tempfile::tempdir().unwrap();
    let fs = Arc::new(FakeFs::with_files([(p("/w/a"), b"stare".to_vec())]));
    let before = fs.snapshot();
    let s = service(fs.clone(), dir.path(), UndoLimits::default(), 1);
    let step = s
        .begin_step(StepCtx::new("s1", Some("Delta"), "x"))
        .unwrap();
    s.write(step, &p("/w/a"), b"nowe").unwrap();
    s.delete_permanent(step, &p("/w/a")).unwrap();
    s.commit_step(step).unwrap();
    drop(s);
    let journal = dir.path().join("journal.ndjson");
    let mut text = std::fs::read_to_string(&journal).unwrap();
    text.push_str("{\"record\":\"begin\",\"ste");
    std::fs::write(&journal, text).unwrap();
    let s = service(fs.clone(), dir.path(), UndoLimits::default(), 2);
    assert_eq!(s.steps(&SessionId::new("s1")).len(), 1);
    s.undo(step).unwrap();
    assert_eq!(fs.snapshot(), before);
    std::fs::write(&journal, "zepsute\n{}\n").unwrap();
    assert!(
        UndoService::open(fs, dir.path(), UndoLimits::default(), Arc::new(|| 0u64), 3).is_err()
    );
}

#[test]
fn corrupted_pre_image_gives_partial_report_never_silent() {
    let dir = tempfile::tempdir().unwrap();
    let fs = Arc::new(FakeFs::with_files([
        (p("/w/a"), b"1".to_vec()),
        (p("/w/b"), b"2".to_vec()),
    ]));
    let s = service(fs.clone(), dir.path(), UndoLimits::default(), 1);
    let step = s.begin_step(StepCtx::new("s1", None, "x")).unwrap();
    s.delete_permanent(step, &p("/w/a")).unwrap();
    s.delete_permanent(step, &p("/w/b")).unwrap();
    s.commit_step(step).unwrap();
    let blob = dir
        .path()
        .join("blobs")
        .join(undo_journal_contract::sha256_hex(b"1"));
    std::fs::write(&blob, b"podmienione").unwrap();
    match s.undo(step) {
        Err(UndoError::Partial(r)) => {
            assert_eq!(r.restored, 1);
            assert_eq!(r.failed.len(), 1);
            assert_eq!(r.failed[0].0, p("/w/a"));
            assert!(r.failed[0].1.contains("uszkodzone"));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(fs.read(&p("/w/b")).unwrap(), b"2");
}

#[test]
fn store_limit_evicts_oldest_steps() {
    let dir = tempfile::tempdir().unwrap();
    let fs = Arc::new(FakeFs::new());
    let limits = UndoLimits {
        store_limit_bytes: 40,
        ..UndoLimits::default()
    };
    let s = service(fs.clone(), dir.path(), limits, 1);
    let mut steps = Vec::new();
    for i in 0..4u8 {
        let path = p(&format!("/w/f{i}"));
        fs.write_atomic(&path, &[i; 15]).unwrap();
        let st = s.begin_step(StepCtx::new("s1", None, "x")).unwrap();
        s.write(st, &path, b"x").unwrap();
        s.commit_step(st).unwrap();
        steps.push(st);
    }
    assert!(
        DirStore::open(dir.path())
            .unwrap()
            .dir()
            .ends_with(dir.path().file_name().unwrap())
    );
    assert!(matches!(s.undo(steps[0]), Err(UndoError::Expired(_))));
    s.undo(steps[3]).unwrap();
    assert_eq!(fs.read(&p("/w/f3")).unwrap(), vec![3u8; 15]);
}

#[tokio::test]
async fn module_lifecycle_and_events() {
    let dir = tempfile::tempdir().unwrap();
    let fs = Arc::new(FakeFs::new());
    let mut s = service(fs, dir.path(), UndoLimits::default(), 1);
    assert_eq!(s.manifest().id.as_str(), "undo-journal");
    assert_eq!(s.health(), HealthStatus::NotStarted);
    let bus = FakeBus::default();
    s.start(ModuleContext::new(
        s.manifest().id.clone(),
        Arc::new(bus.clone()),
    ))
    .await
    .unwrap();
    let step = s
        .begin_step(StepCtx::new("s1", Some("Delta"), "x"))
        .unwrap();
    s.snapshot_scope(step, &p("/w")).unwrap();
    s.write(step, &p("/w/n"), b"1").unwrap();
    s.commit_step(step).unwrap();
    s.undo(step).unwrap();
    assert!(s.undo(step).is_err());
    assert_eq!(s.prune(), 1);
    assert_eq!(s.flush_events().await, 5);
    for name in [
        "undo.snapshot.created",
        "undo.recorded",
        "undo.undone",
        "undo.failed",
        "undo.pruned",
    ] {
        assert_eq!(bus.recorded_of_kind(&event_kind(name)).len(), 1, "{name}");
    }
    assert_eq!(s.health(), HealthStatus::Healthy);
    s.stop().await.unwrap();
}

mod props {
    use super::*;
    use proptest::prelude::*;

    const PATHS: &[&str] = &["/w/a", "/w/b", "/w/d/c", "/w/d/e"];

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        /// ≥ 200 losowych sekwencji na trwałym magazynie, cofanie po restarcie (pre-image z dysku).
        #[test]
        fn random_sequences_on_disk_restore_fully(
            ops in proptest::collection::vec((0u8..5, 0..PATHS.len(), 0..PATHS.len(), proptest::collection::vec(any::<u8>(), 0..8)), 1..15)
        ) {
            let dir = tempfile::tempdir().unwrap();
            let fs = Arc::new(FakeFs::with_files([(p("/w/a"), b"A".to_vec()), (p("/w/d/c"), b"C".to_vec())]));
            let before = fs.snapshot();
            let s = service(fs.clone(), dir.path(), UndoLimits::default(), 1);
            let step = s.begin_step(StepCtx::new("s1", None, "p")).unwrap();
            for (kind, a, b, data) in &ops {
                let (a, b) = (p(PATHS[*a]), p(PATHS[*b]));
                let _ = match kind {
                    0 => s.write(step, &a, data),
                    1 => s.copy(step, &a, &b),
                    2 => s.move_path(step, &a, &b),
                    3 => s.delete(step, &a),
                    _ => s.delete_permanent(step, &a),
                };
            }
            s.commit_step(step).unwrap();
            drop(s);
            let s = service(fs.clone(), dir.path(), UndoLimits::default(), 2);
            s.undo(step).unwrap();
            prop_assert_eq!(fs.snapshot(), before);
        }
    }
}
