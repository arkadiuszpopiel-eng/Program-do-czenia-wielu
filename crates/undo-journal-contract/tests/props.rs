//! ACC-F3-undo-journal-01 / F3-02: losowe sekwencje operacji `fs.*` na `platform-fake`
//! (zapis, nadpisanie, kopia, przeniesienie, Kosz, trwałe usunięcie; także nieudane) w kilku
//! krokach → cofnięcie wszystkich kroków przywraca stan w 100%. Trzy ścieżki odtwarzania:
//! tokeny platformy, wyłącznie pre-image, oraz „restart” (nowe uruchomienie dziennika).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;

use core_bus_contract::SessionId;
use platform_contract::FsPort;
use platform_fake::FakeFs;
use proptest::prelude::*;
use undo_journal_contract::{Journal, JournalStore, MemStore, StepCtx, UndoLimits};

const PATHS: &[&str] = &["/w/a", "/w/b", "/w/c", "/w/d/e", "/w/d/f", "/w/x/y/z"];

#[derive(Debug, Clone)]
enum Op {
    Write(usize, Vec<u8>),
    Copy(usize, usize),
    Move(usize, usize),
    Delete(usize),
    Purge(usize),
}

fn op() -> impl Strategy<Value = Op> {
    let i = || 0..PATHS.len();
    prop_oneof![
        3 => (i(), proptest::collection::vec(any::<u8>(), 0..24)).prop_map(|(p, d)| Op::Write(p, d)),
        1 => (i(), i()).prop_map(|(a, b)| Op::Copy(a, b)),
        2 => (i(), i()).prop_map(|(a, b)| Op::Move(a, b)),
        1 => i().prop_map(Op::Delete),
        1 => i().prop_map(Op::Purge),
    ]
}

/// Pliki początkowe (indeks ścieżki, treść) i kroki z operacjami.
type Scenario = (Vec<(usize, Vec<u8>)>, Vec<Vec<Op>>);

fn scenario() -> impl Strategy<Value = Scenario> {
    (
        proptest::collection::vec(
            (
                0..PATHS.len(),
                proptest::collection::vec(any::<u8>(), 0..16),
            ),
            0..5,
        ),
        proptest::collection::vec(proptest::collection::vec(op(), 1..12), 1..5),
    )
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Platform,
    PreImageOnly,
    Restart,
}

fn run(initial: &[(usize, Vec<u8>)], steps: &[Vec<Op>], mode: Mode) -> Result<(), TestCaseError> {
    let fs = Arc::new(FakeFs::with_files(
        initial
            .iter()
            .map(|(i, d)| (PathBuf::from(PATHS[*i]), d.clone())),
    ));
    let before = fs.snapshot();
    let store: Arc<dyn JournalStore> = Arc::new(MemStore::default());
    let clock = Arc::new(|| 1_000u64);
    let open = |boot| {
        Journal::open(
            fs.clone(),
            store.clone(),
            UndoLimits::default(),
            clock.clone(),
            boot,
        )
        .unwrap()
    };
    let mut j = open(1);
    if matches!(mode, Mode::PreImageOnly) {
        j = j.without_platform_undo();
    }
    let p = |i: usize| PathBuf::from(PATHS[i]);
    for ops in steps {
        let s = j
            .begin_step(StepCtx::new("s1", Some("Delta"), "losowy krok"))
            .unwrap();
        for o in ops {
            let snap = fs.snapshot();
            let r = match o {
                Op::Write(i, d) => j.write(s, &p(*i), d),
                Op::Copy(a, b) => j.copy(s, &p(*a), &p(*b)),
                Op::Move(a, b) => j.move_path(s, &p(*a), &p(*b)),
                Op::Delete(i) => j.delete(s, &p(*i)),
                Op::Purge(i) => j.delete_permanent(s, &p(*i)),
            };
            if r.is_err() {
                prop_assert_eq!(
                    &fs.snapshot(),
                    &snap,
                    "nieudana operacja zmieniła stan: {:?}",
                    o
                );
            }
        }
        j.commit_step(s).unwrap();
    }
    if matches!(mode, Mode::Restart) {
        j = open(2);
    }
    let reports = j.undo_last(&SessionId::new("s1"), steps.len()).unwrap();
    prop_assert_eq!(reports.len(), steps.len());
    prop_assert!(reports.iter().all(|r| r.failed.is_empty()));
    prop_assert_eq!(fs.snapshot(), before);
    let _ = fs.exists(&p(0));
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn undo_restores_state_with_platform_tokens((initial, steps) in scenario()) {
        run(&initial, &steps, Mode::Platform)?;
    }

    #[test]
    fn undo_restores_state_from_pre_images_only((initial, steps) in scenario()) {
        run(&initial, &steps, Mode::PreImageOnly)?;
    }

    #[test]
    fn undo_restores_state_after_restart((initial, steps) in scenario()) {
        run(&initial, &steps, Mode::Restart)?;
    }
}
