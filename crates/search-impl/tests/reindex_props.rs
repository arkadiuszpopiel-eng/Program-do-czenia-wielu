//! Property-based: dowolny przeplot zapisów, usunięć, kroków przebudowy, zmian embeddera i awarii
//! embeddera zbiega do „1 wektor na dokument w aktywnej generacji, 0 brakujących”; wymiana embeddera
//! w działającej usłudze (`set_embedder`, wybór modelu w UI) → przebudowa → wektory nowego modelu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use common::reindex::{Other, count, other, q, seeded, tables};
use search_contract::contract_tests::doc;
use search_contract::{Caller, DocId, DocKind, Mode, Search, SessionId, TxIndexer, VectorStatus};
use search_impl::{ReindexOptions, SqliteSearch};
use sessions_contract::SessionDbProvider;

#[derive(Debug, Clone)]
enum Op {
    Index(u8, u8),
    Remove(u8),
    Step(u8),
    Switch(bool),
    Fail(bool),
}

fn op() -> impl proptest::strategy::Strategy<Value = Op> {
    use proptest::prelude::*;
    prop_oneof![
        3 => (0u8..12, 0u8..4).prop_map(|(k, t)| Op::Index(k, t)),
        1 => (0u8..12).prop_map(Op::Remove),
        2 => (1u8..6).prop_map(Op::Step),
        1 => any::<bool>().prop_map(Op::Switch),
        1 => any::<bool>().prop_map(Op::Fail),
    ]
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(24))]

    /// Dowolny przeplot zapisów, usunięć, kroków, zmian embeddera i awarii: po dokończeniu
    /// przebudowy każdy dokument ma dokładnie jeden wektor w aktywnej generacji, nie ma brakujących
    /// wektorów ani tabel innych generacji.
    #[test]
    fn any_interleaving_converges(ops in proptest::collection::vec(op(), 1..40)) {
        let h = common::harness();
        let flaky = Arc::new(Other::default());
        let other = other(&h, flaky.clone());
        let a = SessionId::new("A");
        let mut use_other = false;
        for op in ops {
            let s: &SqliteSearch = if use_other { &other } else { &h.search };
            match op {
                Op::Index(k, t) => {
                    let text = format!("wpis {k} wariant {t} o górach i morzu");
                    s.index(&doc("A", DocKind::Turn, &k.to_string(), &text)).unwrap();
                }
                Op::Remove(k) => {
                    s.remove(&a, &DocId::new(DocKind::Turn, k.to_string())).unwrap();
                }
                Op::Step(b) => {
                    let db = h.provider.session_db(&a).unwrap();
                    let _ = s.reindex_step(&db, usize::from(b));
                }
                Op::Switch(o) => use_other = o,
                Op::Fail(f) => flaky.fail.store(f, Ordering::SeqCst),
            }
        }
        flaky.fail.store(false, Ordering::SeqCst);
        let s: &SqliteSearch = if use_other { &other } else { &h.search };
        let db = h.provider.session_db(&a).unwrap();
        let p = s.reindex_db(&db, ReindexOptions { batch: 3, pause: std::time::Duration::ZERO }, &AtomicBool::new(false), &mut |_| {}).unwrap();
        proptest::prop_assert!(p.finished, "{p:?}");
        let status = s.vector_status(&a).unwrap();
        proptest::prop_assert!(matches!(status, VectorStatus::Ready { missing: 0, .. }), "{status:?}");
        db.with(|c| {
            let docs = count(c, "SELECT count(*) FROM search_docs");
            let names = tables(c);
            assert_eq!(names.len(), 3, "{names:?}");
            let vectors: i64 = names.iter().map(|t| count(c, &format!("SELECT count(*) FROM {t}"))).sum();
            assert_eq!(vectors, docs);
            assert_eq!(count(c, "SELECT count(*) FROM search_vec_missing"), 0);
            Ok::<(), ()>(())
        })
        .unwrap();
    }
}

#[test]
fn swapping_embedder_in_place_rebuilds_and_switches_queries() {
    let (h, db) = seeded(6);
    let a = SessionId::new("A");
    h.search.set_embedder(Arc::new(Other::default()));
    assert_eq!(h.search.embedder().model_id(), "inny");
    let status = h.search.vector_status(&a).unwrap();
    assert!(
        matches!(
            status,
            VectorStatus::Rebuilding {
                done: 0,
                total: 7,
                ..
            }
        ),
        "{status:?}"
    );
    let opts = ReindexOptions {
        batch: 4,
        pause: std::time::Duration::ZERO,
    };
    let p = h
        .search
        .reindex_db(&db, opts, &AtomicBool::new(false), &mut |_| {})
        .unwrap();
    assert!(p.finished, "{p:?}");
    let ready = VectorStatus::Ready {
        embedder: "inny/8".into(),
        missing: 0,
    };
    assert_eq!(h.search.vector_status(&a).unwrap(), ready);
    let hits = h
        .search
        .query(&q("zielony kolor", Mode::Vector), &Caller::Owner)
        .unwrap();
    assert_eq!(
        hits.first().map(|h| h.doc.clone()),
        Some(DocId::new(DocKind::Memory, "m1"))
    );
    db.with(|c| {
        assert_eq!(count(c, "SELECT count(*) FROM search_vec_turn_g1"), 6);
        Ok::<(), ()>(())
    })
    .unwrap();
}
