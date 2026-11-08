//! Własności `tools-fs` (ACC-F3-tools-fs-01/02): losowe sekwencje operacji przez narzędzia są
//! w 100% cofalne przez dziennik; próby poza zakresem / na deny-liście / z `..` nigdy się nie
//! wykonują (FS i dziennik bez zmian).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use proptest::prelude::*;
use serde_json::{Value, json};
use tools_common_contract::ToolStatus;
use tools_fs_contract::FsToolKind as K;

const NAMES: [&str; 5] = ["a.txt", "b.txt", "sub/c.txt", "sub/d.md", "e.log"];
const DIR: &str = "/Users/ala/Documents";

#[derive(Debug, Clone)]
enum Op {
    Write(usize, String, u8),
    Move(usize, usize),
    Copy(usize, usize),
    Delete(usize),
    Rename(usize, String),
    Mkdir(String),
}

fn op() -> impl Strategy<Value = Op> {
    let i = 0..NAMES.len();
    prop_oneof![
        (i.clone(), "[a-z ]{0,12}", 0u8..3).prop_map(|(i, c, m)| Op::Write(i, c, m)),
        (i.clone(), i.clone()).prop_map(|(a, b)| Op::Move(a, b)),
        (i.clone(), i.clone()).prop_map(|(a, b)| Op::Copy(a, b)),
        i.clone().prop_map(Op::Delete),
        (i, "[a-z]{1,6}\\.txt").prop_map(|(a, n)| Op::Rename(a, n)),
        "[a-z]{1,6}".prop_map(Op::Mkdir),
    ]
}

fn call(op: &Op) -> (K, Value) {
    let p = |i: &usize| format!("{DIR}/{}", NAMES[*i]);
    match op {
        Op::Write(i, c, m) => {
            let mode = ["create", "overwrite", "append"][usize::from(*m)];
            (K::Write, json!({"path": p(i), "content": c, "mode": mode}))
        }
        Op::Move(a, b) => (K::Move, json!({"from": p(a), "to": p(b)})),
        Op::Copy(a, b) => (K::Copy, json!({"from": p(a), "to": p(b)})),
        Op::Delete(i) => (K::Delete, json!({"path": p(i)})),
        Op::Rename(i, n) => (K::Rename, json!({"path": p(i), "new_name": n})),
        Op::Mkdir(d) => (K::Mkdir, json!({"path": format!("{DIR}/{d}")})),
    }
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// ≥ 200 losowych operacji łącznie (50 × 4–10): po cofnięciu wszystkich kroków stan FS
    /// jest identyczny jak przed sekwencją.
    #[test]
    fn random_sequences_are_fully_undoable(ops in proptest::collection::vec(op(), 4..10)) {
        let h = common::harness(&[
            ("/Users/ala/Documents/a.txt", "A"),
            ("/Users/ala/Documents/sub/c.txt", "C"),
            ("/Users/ala/Documents/e.log", "E"),
        ]);
        let before = h.fs.snapshot();
        let committed = rt().block_on(async {
            let mut n = 0usize;
            for op in &ops {
                let (kind, args) = call(op);
                let out = h.call(kind, args).await;
                prop_assert!(matches!(out.status, ToolStatus::Ok | ToolStatus::Failed { .. }), "{op:?}: {out:?}");
                if out.undo.is_some() {
                    n += 1;
                }
            }
            Ok(n)
        })?;
        let steps = h.journal.steps(&"s1".into());
        prop_assert_eq!(steps.iter().filter(|s| s.committed).count(), committed);
        let reports = h.journal.undo_last(&"s1".into(), committed).unwrap();
        prop_assert!(reports.iter().all(|r| r.failed.is_empty()), "{reports:?}");
        prop_assert_eq!(h.fs.snapshot(), before);
    }
}

const FORBIDDEN: [&str; 12] = [
    "/Users/ala/.ssh",
    "/Users/ala/.claude",
    "/Users/ala/.codex",
    "/Users/ala/AppData/Local/Google/Chrome/User Data/Default",
    "/Users/ala/AppData/Roaming/Microsoft/Credentials",
    "/Users/ala/AppData/Roaming/Alfa/kernel",
    "/Users/ala/AppData/Local/Alfa/audit",
    "/ProgramData/AlfaBroker",
    "/Windows/System32",
    "/Users/ala/Documents/../.ssh",
    "/Users/ala/Documents/x.txt:ads",
    "\\\\?\\C:\\Users\\ala",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]

    /// ≥ 100 prób poza zakresem / na deny-liście / z `..` i ADS: 0 wykonanych.
    #[test]
    fn forbidden_targets_never_execute(i in 0..FORBIDDEN.len(), name in "[a-z]{1,8}", kind in 0u8..5) {
        let target = format!("{}/{name}", FORBIDDEN[i]);
        let h = common::harness(&[("/Users/ala/Documents/src.txt", "S")]);
        let before = h.fs.snapshot();
        let (k, args) = match kind {
            0 => (K::Write, json!({"path": target, "content": "x", "mode": "overwrite"})),
            1 => (K::Delete, json!({"path": target})),
            2 => (K::Move, json!({"from": "/Users/ala/Documents/src.txt", "to": target})),
            3 => (K::Copy, json!({"from": "/Users/ala/Documents/src.txt", "to": target})),
            _ => (K::DeletePermanent, json!({"path": target})),
        };
        let out = rt().block_on(h.call(k, args));
        prop_assert!(!out.is_ok(), "{target}: {out:?}");
        prop_assert!(out.undo.is_none());
        prop_assert_eq!(h.fs.snapshot(), before);
        prop_assert!(h.journal.steps(&"s1".into()).is_empty());
    }

    /// Odczyt ścieżek poświadczeń: 0 wykonanych (deny-lista przed Brokerem).
    #[test]
    fn credential_reads_never_execute(i in 0..5usize, name in "[a-z]{1,8}") {
        let target = format!("{}/{name}", FORBIDDEN[i]);
        let h = common::harness(&[(target.as_str(), "SEKRET")]);
        let out = rt().block_on(h.call(K::Read, json!({"path": target})));
        prop_assert!(!out.is_ok());
        prop_assert!(!out.text.contains("SEKRET"));
    }
}
