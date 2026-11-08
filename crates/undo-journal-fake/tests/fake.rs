//! Testy atrapy: kontrakt współdzielony, chaos w trakcie cofania, awaria zapisu dziennika.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use platform_contract::FsPort;
use platform_fake::FakeFs;
use undo_journal_contract::{StepCtx, UndoError, UndoJournal, UndoLimits, contract_tests};
use undo_journal_fake::FakeUndoJournal;

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

#[test]
fn contract_suite() {
    contract_tests::run_all(|limits, clock: Arc<AtomicU64>| {
        let fs = Arc::new(FakeFs::new());
        let c = move || clock.load(Ordering::SeqCst);
        (
            FakeUndoJournal::new(fs.clone(), limits, Arc::new(c)).unwrap(),
            fs as Arc<dyn FsPort>,
        )
    });
}

fn journal(fs: &Arc<FakeFs>) -> FakeUndoJournal {
    let j = FakeUndoJournal::new(fs.clone(), UndoLimits::default(), Arc::new(|| 0u64)).unwrap();
    assert_eq!(j.store().blob_count(), 0);
    j
}

#[test]
fn chaos_during_undo_gives_partial_report_and_keeps_pre_images() {
    let fs = Arc::new(FakeFs::with_files(
        (0..5).map(|i| (p(&format!("/w/f{i}")), vec![i])),
    ));
    let j = journal(&fs);
    let s = j
        .begin_step(StepCtx::new("s1", Some("Delta"), "x"))
        .unwrap();
    for i in 0..5 {
        j.delete_permanent(s, &p(&format!("/w/f{i}"))).unwrap();
    }
    j.commit_step(s).unwrap();
    let blobs = j.store().blob_count();
    j.flaky().arm(2);
    match j.undo(s) {
        Err(UndoError::Partial(r)) => {
            assert_eq!(r.restored, 2);
            assert_eq!(r.failed.len(), 3);
            assert!(r.failed.iter().all(|(_, why)| why.contains("chaos")));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(j.store().blob_count(), blobs, "pre-image nie mogą zginąć");
    assert_eq!(fs.snapshot().len(), 2);
}

#[test]
fn failed_platform_op_or_journal_write_leaves_no_trace() {
    let fs = Arc::new(FakeFs::with_files([(p("/w/a"), b"a".to_vec())]));
    let j = journal(&fs);
    let before = fs.snapshot();
    let s = j.begin_step(StepCtx::new("s1", None, "x")).unwrap();
    j.flaky().arm(0);
    assert!(matches!(
        j.write(s, &p("/w/a"), b"b"),
        Err(UndoError::Platform(_))
    ));
    j.flaky().disarm();
    j.store().set_fail_append(true);
    assert!(matches!(
        j.write(s, &p("/w/a"), b"b"),
        Err(UndoError::Store(_))
    ));
    assert_eq!(
        fs.snapshot(),
        before,
        "operacja bez wpisu w dzienniku została cofnięta"
    );
    j.store().set_fail_append(false);
    j.write(s, &p("/w/a"), b"b").unwrap();
    j.commit_step(s).unwrap();
    j.undo(s).unwrap();
    assert_eq!(fs.snapshot(), before);
    assert!(j.flaky().exists(&p("/w/a")));
}
