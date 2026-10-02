//! Własności rdzenia obserwacji (proptest): dla losowych ciągów operacji na plikach, z losowymi
//! odstępami czasu i okresami utraty zdarzeń (przepełnienie → przeskanowanie) strumień zdarzeń
//! jest spójny (utworzony tylko nieistniejący, usunięty/zmieniony tylko istniejący) i po
//! uspokojeniu odtwarza dokładnie stan katalogu; żadna ścieżka z deny-listy nie wychodzi.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use platform_contract::{
    DirWatchPort, FsChangeKind, WatchEvent, WatchPolicy, WatchSpec, is_temp_name,
};
use platform_fake::FakeDirWatch;
use proptest::prelude::*;

const NAMES: [&str; 9] = [
    "a.txt",
    "b.pdf",
    "c.tmp",
    "sub/d.txt",
    "sub/e.pdf",
    ".ssh/klucz",
    ".claude/auth.json",
    "f.pdf.crdownload",
    "g",
];

#[derive(Debug, Clone)]
enum Op {
    Write(usize, u64),
    Remove(usize),
    Rename(usize, usize),
    Advance(u64),
    Lose,
    Recover,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0..NAMES.len(), 0u64..4).prop_map(|(n, l)| Op::Write(n, l)),
        2 => (0..NAMES.len()).prop_map(Op::Remove),
        2 => (0..NAMES.len(), 0..NAMES.len()).prop_map(|(a, b)| Op::Rename(a, b)),
        3 => (0u64..2_000).prop_map(Op::Advance),
        1 => Just(Op::Lose),
        1 => Just(Op::Recover),
    ]
}

fn root() -> PathBuf {
    PathBuf::from("/w")
}

fn path(i: usize) -> PathBuf {
    root().join(NAMES[i])
}

fn relevant(p: &Path) -> bool {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    !is_temp_name(&name) && !WatchPolicy::baseline().is_denied(p)
}

fn apply(model: &mut BTreeSet<PathBuf>, ev: &WatchEvent) -> Result<(), String> {
    let WatchEvent::Changed { path, change, .. } = ev else {
        return Ok(());
    };
    if WatchPolicy::baseline().is_denied(path) {
        return Err(format!("ścieżka z deny-listy wyszła: {}", path.display()));
    }
    let ok = match change {
        FsChangeKind::Created => model.insert(path.clone()),
        FsChangeKind::Removed => model.remove(path),
        FsChangeKind::Modified => model.contains(path),
        FsChangeKind::Renamed { from } => {
            !WatchPolicy::baseline().is_denied(from)
                && model.remove(from)
                && model.insert(path.clone())
        }
    };
    if ok {
        Ok(())
    } else {
        Err(format!("niespójne zdarzenie {ev:?} przy {model:?}"))
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn stream_is_consistent_and_converges(
        seed in proptest::collection::vec(any::<bool>(), NAMES.len()),
        ops in proptest::collection::vec(op(), 1..60),
    ) {
        let w = FakeDirWatch::default();
        for (i, on) in seed.iter().enumerate() {
            if *on {
                w.seed(path(i), 1);
            }
        }
        w.watch(WatchSpec::new(root()).recursive()).unwrap();
        let mut model: BTreeSet<PathBuf> =
            w.files().into_iter().filter(|p| relevant(p)).collect();
        let mut events = Vec::new();
        let mut losing = false;
        for op in ops {
            match op {
                Op::Write(n, l) => w.write(path(n), l),
                Op::Remove(n) => w.remove(&path(n)),
                Op::Rename(a, b) => w.rename(&path(a), path(b)),
                Op::Advance(ms) => w.advance(ms),
                Op::Lose => {
                    losing = true;
                    w.lose_events(true);
                }
                Op::Recover if losing => {
                    losing = false;
                    w.lose_events(false);
                    w.overflow();
                }
                Op::Recover => {}
            }
            events.extend(w.drain_events());
        }
        w.lose_events(false);
        w.overflow();
        w.advance(120_000);
        events.extend(w.drain_events());
        for ev in &events {
            if let Err(e) = apply(&mut model, ev) {
                prop_assert!(false, "{}", e);
            }
        }
        let expected: BTreeSet<PathBuf> =
            w.files().into_iter().filter(|p| relevant(p)).collect();
        prop_assert_eq!(model, expected);
    }
}
