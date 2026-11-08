//! Współdzielone testy kontraktowe (feature `contract-tests`) dla `-impl` i `-fake`.
//! Fabryka dostaje limity i wirtualny zegar, zwraca dziennik i jego `FsPort` (pliki pod `/w`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use core_bus_contract::SessionId;
use platform_contract::FsPort;

use crate::{StepCtx, UndoError, UndoJournal, UndoLimits};

/// Migawka plików pod `root` (rekurencyjnie, przez `FsPort`).
pub fn tree(fs: &dyn FsPort, root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs.list_dir(&dir) else {
            continue;
        };
        for e in entries {
            if e.is_dir {
                stack.push(e.path);
            } else if let Ok(d) = fs.read(&e.path) {
                out.insert(e.path, d);
            }
        }
    }
    out
}

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

fn ok<T>(r: Result<T, UndoError>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn seed(fs: &dyn FsPort) {
    for (path, data) in [("/w/a.txt", "a"), ("/w/b.txt", "bb"), ("/w/d/c.txt", "ccc")] {
        ok(fs
            .write_atomic(&p(path), data.as_bytes())
            .map_err(UndoError::Platform));
    }
}

fn ctx() -> StepCtx {
    StepCtx::new("s1", Some("Delta"), "porządki")
}

/// Krok z każdą operacją → cofnięcie przywraca stan; podsumowanie po polsku.
pub fn every_op_undone<J: UndoJournal>(j: &J, fs: &dyn FsPort) {
    seed(fs);
    let before = tree(fs, &p("/w"));
    let s = ok(j.begin_step(ctx()));
    ok(j.write(s, &p("/w/a.txt"), b"nowa"));
    ok(j.write(s, &p("/w/new.txt"), b"n"));
    ok(j.copy(s, &p("/w/b.txt"), &p("/w/b2.txt")));
    ok(j.move_path(s, &p("/w/d/c.txt"), &p("/w/c-moved.txt")));
    ok(j.delete(s, &p("/w/b.txt")));
    ok(j.delete_permanent(s, &p("/w/new.txt")));
    let sum = ok(j.commit_step(s));
    assert_eq!(sum.counts.written, 2);
    assert!(
        sum.reversible && sum.text.starts_with("Delta: zapisano 2 pliki"),
        "{}",
        sum.text
    );
    let report = ok(j.undo(s));
    assert_eq!(report.restored, 6);
    assert_eq!(tree(fs, &p("/w")), before);
    assert!(matches!(j.undo(s), Err(UndoError::BadState { .. })));
    assert!(matches!(
        j.undo(crate::StepId(999)),
        Err(UndoError::UnknownStep(_))
    ));
}

/// Plik zmieniony po operacji → konflikt, nic nie jest ruszane.
pub fn conflict_blocks_undo<J: UndoJournal>(j: &J, fs: &dyn FsPort) {
    seed(fs);
    let s = ok(j.begin_step(ctx()));
    ok(j.write(s, &p("/w/a.txt"), b"agentka"));
    ok(j.delete(s, &p("/w/b.txt")));
    ok(j.commit_step(s));
    ok(fs
        .write_atomic(&p("/w/a.txt"), b"uzytkownik")
        .map_err(UndoError::Platform));
    let after_user = tree(fs, &p("/w"));
    match j.undo(s) {
        Err(UndoError::Conflict { path, .. }) => assert_eq!(path, p("/w/a.txt")),
        other => panic!("oczekiwano konfliktu, jest {other:?}"),
    }
    assert_eq!(tree(fs, &p("/w")), after_user);
    let msg = UndoError::Conflict {
        path: p("/w/a.txt"),
        expected: crate::FileState::Absent,
        found: crate::FileState::Absent,
    }
    .to_string();
    assert!(msg.contains("zmieniono po tej operacji"));
}

/// `undo_last(n)` cofa od najnowszego; `abort_step` cofa niezatwierdzony krok.
pub fn undo_last_and_abort<J: UndoJournal>(j: &J, fs: &dyn FsPort) {
    seed(fs);
    let s1 = ok(j.begin_step(ctx()));
    ok(j.write(s1, &p("/w/a.txt"), b"1"));
    ok(j.commit_step(s1));
    let after1 = tree(fs, &p("/w"));
    for i in 2..=3u8 {
        let s = ok(j.begin_step(ctx()));
        ok(j.write(s, &p("/w/a.txt"), &[i]));
        ok(j.write(s, &p(&format!("/w/n{i}.txt")), &[i]));
        ok(j.commit_step(s));
    }
    let reports = ok(j.undo_last(&SessionId::new("s1"), 2));
    assert_eq!(reports.len(), 2);
    assert_eq!(tree(fs, &p("/w")), after1);
    let s = ok(j.begin_step(ctx()));
    ok(j.write(s, &p("/w/x.txt"), b"x"));
    ok(j.abort_step(s));
    assert_eq!(tree(fs, &p("/w")), after1);
    assert_eq!(j.steps(&SessionId::new("s1")).len(), 4);
}

/// Snapshot zakresu dla shella: dowolne zmiany skryptu w zakresie są cofane.
pub fn shell_scope_snapshot<J: UndoJournal>(j: &J, fs: &dyn FsPort) {
    seed(fs);
    let before = tree(fs, &p("/w"));
    let s = ok(j.begin_step(ctx()));
    ok(j.snapshot_scope(s, &p("/w")));
    let sh = |r: Result<platform_contract::OpReceipt, platform_contract::PlatformError>| {
        ok(r.map_err(UndoError::Platform))
    };
    sh(fs.write_atomic(&p("/w/d/gen.o"), b"obj"));
    sh(fs.write_atomic(&p("/w/a.txt"), b"zmienione"));
    sh(fs.delete_permanent(&p("/w/b.txt")));
    ok(j.commit_step(s));
    ok(j.undo(s));
    assert_eq!(tree(fs, &p("/w")), before);
}

/// Pre-image ponad limit bez zgody → operacja nie wykonana; z zgodą → nieodwracalna.
pub fn pre_image_limit<J: UndoJournal>(j: &J, fs: &dyn FsPort) {
    let big = vec![7u8; 64];
    ok(fs
        .write_atomic(&p("/w/big.bin"), &big)
        .map_err(UndoError::Platform));
    let s = ok(j.begin_step(ctx()));
    assert!(matches!(
        j.write(s, &p("/w/big.bin"), b"x"),
        Err(UndoError::PreImageTooLarge { .. })
    ));
    assert!(matches!(
        j.delete_permanent(s, &p("/w/big.bin")),
        Err(UndoError::PreImageTooLarge { .. })
    ));
    assert_eq!(fs.read(&p("/w/big.bin")).ok(), Some(big));
    let mut allowed = ctx();
    allowed.allow_irreversible = true;
    let s2 = ok(j.begin_step(allowed));
    ok(j.delete_permanent(s2, &p("/w/big.bin")));
    assert!(!ok(j.commit_step(s2)).reversible);
}

/// Retencja: po terminie krok jest przeterminowany i nie da się go cofnąć.
pub fn retention_expires<J: UndoJournal>(
    j: &J,
    fs: &dyn FsPort,
    clock: &AtomicU64,
    limits: &UndoLimits,
) {
    seed(fs);
    let s = ok(j.begin_step(ctx()));
    ok(j.write(s, &p("/w/a.txt"), b"z"));
    ok(j.commit_step(s));
    assert_eq!(j.prune(), 0);
    clock.fetch_add(limits.retention_ms, Ordering::SeqCst);
    assert_eq!(j.prune(), 1);
    assert!(matches!(j.undo(s), Err(UndoError::Expired(_))));
}

/// Limity testowe: pre-image do 32 B.
pub fn test_limits() -> UndoLimits {
    UndoLimits {
        pre_image_max_bytes: 32,
        ..UndoLimits::default()
    }
}

/// Uruchamia zestaw; `factory(limity, zegar)` daje świeży dziennik i jego `FsPort`.
pub fn run_all<J, F>(factory: F)
where
    J: UndoJournal,
    F: Fn(UndoLimits, Arc<AtomicU64>) -> (J, Arc<dyn FsPort>),
{
    let limits = test_limits();
    let fresh = || {
        let clock = Arc::new(AtomicU64::new(1_000));
        let (j, fs) = factory(limits, clock.clone());
        (j, fs, clock)
    };
    let (j, fs, _) = fresh();
    every_op_undone(&j, fs.as_ref());
    let (j, fs, _) = fresh();
    conflict_blocks_undo(&j, fs.as_ref());
    let (j, fs, _) = fresh();
    undo_last_and_abort(&j, fs.as_ref());
    let (j, fs, _) = fresh();
    shell_scope_snapshot(&j, fs.as_ref());
    let (j, fs, _) = fresh();
    pre_image_limit(&j, fs.as_ref());
    let (j, fs, clock) = fresh();
    retention_expires(&j, fs.as_ref(), &clock, &limits);
}
