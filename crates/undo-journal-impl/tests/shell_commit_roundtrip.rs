//! Nonempty shell post-state maps must survive the persisted tagged-enum JSON format.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use platform_contract::FsPort;
use platform_fake::FakeFs;
use std::collections::BTreeMap;
use std::{path::PathBuf, sync::Arc};
use undo_journal_contract::{
    JournalRecord, JournalStore, StepCtx, StepId, UndoJournal, UndoLimits,
};
use undo_journal_impl::{DirStore, UndoService};
#[test]
fn nonempty_commit_round_trips_existing_json_keys() {
    let dir = tempfile::tempdir().unwrap();
    let record = JournalRecord::Commit {
        step: StepId(1),
        at_ms: 5,
        posts: [(0, BTreeMap::new())].into(),
    };
    {
        let store = DirStore::open(dir.path()).unwrap();
        store.append(&record).unwrap();
    }
    let store = DirStore::open(dir.path()).unwrap();
    assert_eq!(store.load().unwrap(), vec![record]);
}
#[test]
fn shell_snapshot_can_be_reopened_and_undone() {
    let dir = tempfile::tempdir().unwrap();
    let path = PathBuf::from("/work/a.txt");
    let fs = Arc::new(FakeFs::with_files([(path.clone(), b"before".to_vec())]));
    let service = UndoService::open(
        fs.clone(),
        dir.path(),
        UndoLimits::default(),
        Arc::new(|| 10u64),
        1,
    )
    .unwrap();
    let step = service
        .begin_step(StepCtx::new("s1", Some("Delta"), "shell test"))
        .unwrap();
    service.snapshot_scope(step, "/work".as_ref()).unwrap();
    fs.write_atomic(&path, b"after").unwrap();
    service.commit_step(step).unwrap();
    drop(service);
    let service = UndoService::open(
        fs.clone(),
        dir.path(),
        UndoLimits::default(),
        Arc::new(|| 20u64),
        2,
    )
    .unwrap();
    service.undo(step).unwrap();
    assert_eq!(fs.read(&path).unwrap(), b"before");
}

#[test]
fn malformed_post_indices_are_rejected() {
    for entries in [
        r#""-1":{}"#,
        r#""01":{}"#,
        r#""4294967296":{}"#,
        r#""0":{},"0":{}"#,
    ] {
        let raw = format!(r#"{{"record":"commit","step":1,"at_ms":5,"posts":{{{entries}}}}}"#);
        assert!(
            serde_json::from_str::<JournalRecord>(&raw).is_err(),
            "{raw}"
        );
    }
    let record = JournalRecord::Commit {
        step: StepId(1),
        at_ms: 5,
        posts: [(0, BTreeMap::new())].into(),
    };
    assert!(
        serde_json::to_string(&record)
            .unwrap()
            .contains(r#""posts":{"0":{}}"#)
    );
}
