//! Property (F7-03): po `forget(źródło)` **żadne** zapytanie nie zwraca treści z tego źródła —
//! `recall` (każdy zakres), Inspektor (lista i wyszukiwanie), indeks `search` (FTS, wektor,
//! hybryda) i surowe tabele baz. Treść pochodnych (kopie, streszczenia, edycje) niesie znacznik
//! źródła, więc znacznik nie może się pojawić nigdzie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use lib_sqlstore::rusqlite::Connection;
use memory_contract::{
    Accessor, ChangeOp, ChangeSet, Derivation, EntryEdit, EntryRef, ForgetTarget, InspectorQuery,
    Layer, MemoryScope, MemoryService, NewMemory, Origin, Provenance, RecallRequest, RememberMode,
    SessionId,
};
use proptest::prelude::*;
use search_contract::{Caller, DocKind, Mode, Query, Search, SessionSet};

#[derive(Debug, Clone)]
enum Op {
    Fact(usize),
    Untrusted(usize, usize),
    Promote(usize),
    Summary(usize, usize),
    Edit(usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..4_usize).prop_map(Op::Fact),
        (0..4_usize, 0..2_usize).prop_map(|(s, u)| Op::Untrusted(s, u)),
        (0..64_usize).prop_map(Op::Promote),
        (0..64_usize, 0..64_usize).prop_map(|(a, b)| Op::Summary(a, b)),
        (0..64_usize).prop_map(Op::Edit),
    ]
}

/// Wpis w modelu testu: odwołanie + znaczniki źródeł (sesja/URL), z których pochodzi treść.
#[derive(Clone)]
struct Tracked {
    entry: EntryRef,
    session: usize,
    url: Option<usize>,
    markers: Vec<String>,
}

fn url(u: usize) -> String {
    format!("https://zrodlo{u}.test/strona")
}

fn raw_contains(conn: &Connection, needle: &str) -> bool {
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'memory_%'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    tables.iter().any(|t| {
        let n: i64 = conn
            .query_row(
                &format!(
                    "SELECT count(*) FROM \"{t}\" WHERE CAST({} AS TEXT) LIKE ?1",
                    if t == "memory_exports" {
                        "name"
                    } else {
                        "body"
                    }
                ),
                [format!("%{needle}%")],
                |r| r.get(0),
            )
            .unwrap();
        n > 0
    })
}

fn build(s: &common::Stack, ops: &[Op]) -> Vec<Tracked> {
    let owner = Accessor::Owner;
    let mut all: Vec<Tracked> = Vec::new();
    for (n, op) in ops.iter().enumerate() {
        let marker = format!("znk{n}x");
        match op {
            Op::Fact(si) | Op::Untrusted(si, _) => {
                let sid = SessionId::new(format!("S{si}"));
                let (prov, u) = match op {
                    Op::Untrusted(_, u) => {
                        (Provenance::UntrustedContent { source: url(*u) }, Some(*u))
                    }
                    _ => (Provenance::User, None),
                };
                let new = NewMemory {
                    origin: Origin::from_turn(sid.clone(), n as u64),
                    ..NewMemory::new(
                        MemoryScope::Session(sid),
                        Layer::Semantic,
                        format!("Fakt {marker} o rzeczy"),
                        prov,
                    )
                };
                let e = s.remember_as(&owner, new, RememberMode::Explicit).unwrap();
                all.push(Tracked {
                    entry: e.entry_ref(),
                    session: *si,
                    url: u,
                    markers: vec![marker],
                });
            }
            Op::Promote(i) if !all.is_empty() => {
                let src = all[i % all.len()].clone();
                if let Ok(c) = s.promote_as(&owner, &src.entry, MemoryScope::Global) {
                    all.push(Tracked {
                        entry: c.entry_ref(),
                        ..src
                    });
                }
            }
            Op::Summary(a, b) if !all.is_empty() => {
                let (x, y) = (all[a % all.len()].clone(), all[b % all.len()].clone());
                if x.entry.scope != y.entry.scope || x.entry == y.entry {
                    continue;
                }
                let mut markers = x.markers.clone();
                markers.extend(y.markers.clone());
                let new = NewMemory {
                    origin: Origin::derived(
                        Derivation::Summary,
                        vec![x.entry.clone(), y.entry.clone()],
                    ),
                    ..NewMemory::new(
                        x.entry.scope.clone(),
                        Layer::Episodic,
                        format!("Streszczenie {}", markers.join(" ")),
                        Provenance::User,
                    )
                };
                let set = ChangeSet {
                    scope: x.entry.scope.clone(),
                    run: format!("r{n}"),
                    ops: vec![ChangeOp::Create {
                        entry: new,
                        approved: true,
                        note: "s".into(),
                    }],
                };
                if let Ok(r) = s.apply_changes(&Accessor::Guardian, &set) {
                    all.push(Tracked {
                        entry: r.created[0].clone(),
                        session: x.session,
                        url: x.url.or(y.url),
                        markers,
                    });
                }
            }
            Op::Edit(i) if !all.is_empty() => {
                let src = all[i % all.len()].clone();
                let edit = EntryEdit {
                    text: Some(format!("Edytowane {}", src.markers.join(" "))),
                    ..EntryEdit::default()
                };
                if let Ok(e) = s.edit(&owner, &src.entry, &edit) {
                    all.push(Tracked {
                        entry: e.entry_ref(),
                        ..src
                    });
                }
            }
            _ => {}
        }
    }
    all
}

fn assert_marker_gone(s: &common::Stack, marker: &str) {
    let owner = Accessor::Owner;
    let mut scopes: Vec<MemoryScope> = (0..4)
        .map(|i| MemoryScope::Session(SessionId::new(format!("S{i}"))))
        .collect();
    scopes.push(MemoryScope::Global);
    let hits = s
        .recall_as(&owner, &RecallRequest::new(scopes.clone(), marker, 50))
        .unwrap();
    assert!(
        hits.iter().all(|h| !h.entry.text.contains(marker)),
        "recall: {marker}"
    );
    let page = s
        .inspect(
            &owner,
            &InspectorQuery {
                limit: 500,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        page.items.iter().all(|i| !i.entry.text.contains(marker)),
        "Inspektor: {marker}"
    );
    for mode in [Mode::Fts, Mode::Vector, Mode::Hybrid] {
        let q = Query {
            text: marker.into(),
            sessions: SessionSet::All,
            mode,
            limit: 200,
            kinds: vec![DocKind::Memory],
        };
        let hits = s.index.query(&q, &Caller::Owner).unwrap();
        assert!(
            hits.iter().all(|h| !h.snippet.text.contains(marker)),
            "indeks {mode:?}: {marker}"
        );
    }
    for scope in &scopes {
        if let Some(db) = memory_impl::ScopeDbs::db(s.dbs.as_ref(), scope, false).unwrap() {
            db.with(|c| {
                assert!(
                    !raw_contains(c, marker),
                    "surowe tabele {scope:?}: {marker}"
                );
                Ok::<(), ()>(())
            })
            .unwrap();
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn forget_source_leaves_no_trace(ops in proptest::collection::vec(op(), 4..18), pick in 0..3_usize, which in 0..64_usize) {
        let s = common::stack();
        let all = build(&s, &ops);
        let roots: Vec<&Tracked> = all
            .iter()
            .enumerate()
            .filter(|(i, t)| t.markers.len() == 1 && !all[..*i].iter().any(|o| o.markers == t.markers))
            .map(|(_, t)| t)
            .collect();
        prop_assume!(!roots.is_empty());
        let chosen = roots[which % roots.len()].clone();
        let in_roots = |pred: &dyn Fn(&Tracked) -> bool| -> Vec<String> {
            roots.iter().filter(|t| pred(t)).map(|t| t.markers[0].clone()).collect()
        };
        let (target, forgotten) = match (pick, chosen.url) {
            (0, _) => (
                ForgetTarget::Session(SessionId::new(format!("S{}", chosen.session))),
                in_roots(&|t| t.session == chosen.session),
            ),
            (1, Some(u)) => (ForgetTarget::Source(url(u)), in_roots(&|t| t.url == Some(u))),
            _ => (ForgetTarget::Entry(chosen.entry.clone()), vec![chosen.markers[0].clone()]),
        };
        let report = s.forget_as(&Accessor::Owner, &target).unwrap();
        prop_assert!(!report.removed.is_empty());
        for marker in &forgotten {
            assert_marker_gone(&s, marker);
        }
    }
}
