//! Zmiana embeddera (F7-02): przebudowa wektorów w tle na atrapach (`HashEmbedder` 64 wym. →
//! embedder 8 wym.) — stan, FTS w trakcie, zapisy/usunięcia w trakcie, kroki z postępem, wznowienie
//! po przerwaniu, porzucenie przy powrocie do starego embeddera, brakujące wektory przy awarii
//! embeddera, ochrona przed nadpisaniem zmienionego dokumentu, wątek tła ze zdarzeniami.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use common::reindex::{Other, count, other, q, seeded, tables};
use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use core_registry_contract::{Module, ModuleContext};
use lib_sqlstore::Db;
use search_contract::contract_tests::doc;
use search_contract::{
    Caller, DocId, DocKind, Mode, Search, SearchError, SessionId, TxIndexer, VectorStatus, events,
};
use search_impl::{ReindexOptions, ReindexSource, SqliteSearch};
use sessions_contract::SessionDbProvider;

#[test]
fn embedder_change_rebuilds_with_fts_fallback() {
    let (h, db) = seeded(20);
    let s = other(&h, Arc::new(Other::default()));
    let a = SessionId::new("A");
    // Zapis w trakcie nie zawodzi (dawniej `EmbedderMismatch`) — trafia do nowej generacji.
    s.index(&doc("A", DocKind::Turn, "nowy", "nowy dokument o żaglach"))
        .unwrap();
    let status = s.vector_status(&a).unwrap();
    assert!(
        matches!(&status, VectorStatus::Rebuilding { from, to, done: 0, total: 22 }
        if from == "fake-hash-ngram-64/64" && to == "inny/8"),
        "{status:?}"
    );
    // Zapytania wektorowe i hybrydowe działają jak FTS.
    for mode in [Mode::Vector, Mode::Hybrid, Mode::Fts] {
        let hits = s.query(&q("zielony", mode), &Caller::Owner).unwrap();
        assert_eq!(hits.len(), 1, "{mode:?}");
        assert_eq!(hits[0].doc, DocId::new(DocKind::Memory, "m1"));
    }
    assert!(
        s.query(&q("xyzzy", Mode::Vector), &Caller::Owner)
            .unwrap()
            .is_empty()
    );
    s.remove(&a, &DocId::new(DocKind::Turn, "3")).unwrap();
    let mut seen = Vec::new();
    loop {
        let p = s.reindex_step(&db, 7).unwrap();
        seen.push(p);
        if p.finished {
            break;
        }
        assert!(seen.len() < 20, "przebudowa się nie kończy: {seen:?}");
    }
    assert!(seen.windows(2).all(|w| w[0].done <= w[1].done));
    assert_eq!(seen.iter().map(|p| p.embedded).sum::<u64>(), 21);
    assert_eq!(
        s.vector_status(&a).unwrap(),
        VectorStatus::Ready {
            embedder: "inny/8".into(),
            missing: 0
        }
    );
    db.with(|c| {
        assert_eq!(
            tables(c),
            [
                "search_vec_artifact_g1",
                "search_vec_memory_g1",
                "search_vec_turn_g1"
            ]
        );
        assert_eq!(count(c, "SELECT count(*) FROM search_vec_turn_g1"), 20);
        assert_eq!(count(c, "SELECT count(*) FROM search_vec_missing"), 0);
        Ok::<(), ()>(())
    })
    .unwrap();
    // Po przebudowie wektory znów działają: kNN zwraca także dokumenty bez dopasowania FTS.
    let hits = s.query(&q("xyzzy", Mode::Vector), &Caller::Owner).unwrap();
    assert_eq!(hits.len(), 5);
}

#[test]
fn rebuild_resumes_after_restart() {
    let (h, db) = seeded(30);
    let first = other(&h, Arc::new(Other::default()));
    first.reindex_step(&db, 10).unwrap();
    let p = first.reindex_step(&db, 10).unwrap();
    assert_eq!((p.done, p.total, p.finished), (20, 31, false));
    drop(first);
    let second = other(&h, Arc::new(Other::default()));
    assert!(matches!(
        second.vector_status(&SessionId::new("A")).unwrap(),
        VectorStatus::Rebuilding { done: 20, .. }
    ));
    let p = second
        .reindex_db(
            &db,
            ReindexOptions {
                batch: 10,
                ..Default::default()
            },
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
    assert!(p.finished && p.done == 31);
}

#[test]
fn switching_back_abandons_rebuild_and_fills_gaps() {
    let (h, db) = seeded(10);
    let s = other(&h, Arc::new(Other::default()));
    s.reindex_step(&db, 4).unwrap();
    s.index(&doc(
        "A",
        DocKind::Turn,
        "w-trakcie",
        "dopisany w trakcie przebudowy",
    ))
    .unwrap();
    // Powrót do `HashEmbedder`: generacja docelowa porzucona, stara aktywna; dopisany dokument
    // nie ma w niej wektora → `missing`, który uzupełnia krok.
    let back = &h.search;
    let a = SessionId::new("A");
    assert_eq!(
        back.vector_status(&a).unwrap(),
        VectorStatus::Ready {
            embedder: "fake-hash-ngram-64/64".into(),
            missing: 1
        }
    );
    db.with(|c| {
        assert!(tables(c).iter().all(|t| !t.ends_with("_g1")));
        Ok::<(), ()>(())
    })
    .unwrap();
    let p = back.reindex_step(&db, 8).unwrap();
    assert!(p.finished && p.embedded == 1);
    let hits = back
        .query(&q("dopisany przebudowy", Mode::Vector), &Caller::Owner)
        .unwrap();
    assert_eq!(hits[0].doc.key, "w-trakcie");
}

#[test]
fn embedder_failure_keeps_fts_and_vector_is_filled_later() {
    let h = common::harness();
    let embedder = Arc::new(Other::default());
    let s = other(&h, embedder.clone());
    let a = SessionId::new("A");
    s.index(&doc("A", DocKind::Turn, "1", "pierwszy tekst o górach"))
        .unwrap();
    embedder.fail.store(true, Ordering::SeqCst);
    s.index(&doc("A", DocKind::Turn, "2", "drugi tekst o morzu"))
        .unwrap();
    assert_eq!(
        s.vector_status(&a).unwrap(),
        VectorStatus::Ready {
            embedder: "inny/8".into(),
            missing: 1
        }
    );
    // Embedder zapytania też niedostępny → hybryda i wektor spadają do FTS (bez błędu).
    let hits = s.query(&q("morzu", Mode::Hybrid), &Caller::Owner).unwrap();
    assert_eq!(hits[0].doc.key, "2");
    let db = h.provider.session_db(&a).unwrap();
    assert!(matches!(
        s.reindex_step(&db, 8),
        Err(SearchError::Embedder { .. })
    ));
    embedder.fail.store(false, Ordering::SeqCst);
    let vector_only = s.query(&q("tekst", Mode::Vector), &Caller::Owner).unwrap();
    assert_eq!(vector_only.len(), 1, "dokument bez wektora poza kNN");
    let p = s.reindex_step(&db, 8).unwrap();
    assert!(p.finished && p.embedded == 1 && p.done == 2);
    assert_eq!(
        s.query(&q("tekst", Mode::Vector), &Caller::Owner)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn document_changed_during_embedding_is_not_overwritten() {
    let (h, db) = seeded(3);
    let embedder = Arc::new(Other::default());
    let s = other(&h, embedder.clone());
    s.vector_status(&SessionId::new("A")).unwrap();
    let writer = other(&h, Arc::new(Other::default()));
    *embedder.hook.lock().unwrap() = Some(Box::new(move || {
        writer
            .index(&doc("A", DocKind::Turn, "1", "zupełnie nowa treść"))
            .unwrap();
    }));
    let p = s.reindex_step(&db, 10).unwrap();
    assert_eq!(p.embedded, 3, "pominięty zmieniony dokument (4 w partii)");
    let rowid: i64 = db
        .with(|c| {
            c.query_row("SELECT id FROM search_docs WHERE key = '1'", [], |r| {
                r.get(0)
            })
        })
        .unwrap();
    let stored: Vec<u8> = db
        .with(|c| {
            c.query_row(
                "SELECT embedding FROM search_vec_turn_g1 WHERE rowid = ?1",
                [rowid],
                |r| r.get(0),
            )
        })
        .unwrap();
    assert_eq!(
        stored,
        lib_sqlstore::vector_to_blob(&Other::vector("zupełnie nowa treść"))
    );
}

struct Extra(Arc<Db>);

impl ReindexSource for Extra {
    fn databases(&self) -> Result<Vec<(SessionId, Arc<Db>)>, SearchError> {
        Ok(vec![(SessionId::new("zakres-globalny"), self.0.clone())])
    }
}

#[tokio::test]
async fn background_rebuild_over_sessions_and_extra_sources() {
    let (h, _db) = seeded(12);
    h.index(&doc("B", DocKind::Turn, "1", "sesja B")).unwrap();
    // Baza „zakresu pamięci” indeksowana przez `TxIndexer` (jak `memory-impl`).
    let scope = h
        .provider
        .session_db(&SessionId::new("zakres-globalny"))
        .unwrap();
    scope
        .with(|c| {
            h.search.prepare(c)?;
            h.search.index_in(
                c,
                &doc("zakres-globalny", DocKind::Memory, "f1", "fakt globalny"),
            )
        })
        .unwrap();
    let mut s = SqliteSearch::new(h.provider.clone(), Arc::new(Other::default())).unwrap();
    let bus = FakeBus::default();
    s.start(ModuleContext::new(
        s.manifest().id.clone(),
        Arc::new(bus.clone()),
    ))
    .await
    .unwrap();
    let s = Arc::new(s);
    let calls = Arc::new(Mutex::new(0_usize));
    let counter = calls.clone();
    let handle = s
        .spawn_reindex(
            vec![Arc::new(Extra(scope.clone()))],
            ReindexOptions {
                batch: 5,
                pause: std::time::Duration::ZERO,
            },
            Some(Arc::new(move |_| *counter.lock().unwrap() += 1)),
        )
        .unwrap();
    let report = tokio::task::spawn_blocking(move || handle.join())
        .await
        .unwrap();
    assert!(
        report.finished && !report.cancelled && report.failed.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.embedded, 13 + 1 + 1);
    assert!(report.databases >= 3 && report.rebuilt == 3);
    assert!(*calls.lock().unwrap() > 3);
    scope
        .with(|c| {
            assert!(s.vector_status_in(c)?.vectors_usable());
            Ok::<(), SearchError>(())
        })
        .unwrap();
    let kinds = [
        events::REINDEX_STARTED,
        events::REINDEX_PROGRESS,
        events::REINDEX_DONE,
    ];
    for kind in kinds {
        let k = EventKind::Custom(kind.to_owned());
        for _ in 0..200 {
            if !bus.recorded_of_kind(&k).is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let recorded = bus.recorded_of_kind(&k);
        assert!(!recorded.is_empty(), "{kind}");
        assert!(
            recorded
                .iter()
                .all(|e| !e.payload.to_string().contains("jeziorze"))
        );
    }
    // Drugi przebieg: nic do zrobienia, anulowanie natychmiastowe działa.
    let again = s
        .spawn_reindex(Vec::new(), ReindexOptions::default(), None)
        .unwrap();
    again.cancel();
    let r = again.join();
    assert!(r.finished && r.embedded == 0);
}

#[test]
fn default_tx_indexer_has_nothing_to_rebuild() {
    let h = common::harness();
    let db = h.provider.session_db(&SessionId::new("A")).unwrap();
    for indexer in [
        &search_fake::FakeSearch::new() as &dyn TxIndexer,
        &search_fake::RecordingIndexer::new(),
    ] {
        let p = indexer.reindex_step(&db, 4).unwrap();
        assert!(p.finished && p.embedded == 0);
        let status = db.with(|c| indexer.vector_status_in(c)).unwrap();
        assert!(status.vectors_usable() && !status.needs_work());
    }
}

#[test]
fn poisoned_document_does_not_stall_rebuild() {
    let (h, db) = seeded(6);
    h.index(&doc(
        "A",
        DocKind::Turn,
        "zly",
        "tekst z trucizną: trucizna",
    ))
    .unwrap();
    let embedder = Arc::new(Other::default());
    embedder.poison.store(true, Ordering::SeqCst);
    let s = other(&h, embedder);
    let mut last = Ok(search_contract::ReindexProgress::default());
    for _ in 0..10 {
        last = s.reindex_step(&db, 4);
        if last.is_err() {
            break;
        }
    }
    // Przebudowa doszła do końca; zostaje jeden dokument bez wektora, a krok zgłasza błąd embeddera.
    assert!(
        matches!(last, Err(SearchError::Embedder { .. })),
        "{last:?}"
    );
    assert_eq!(
        s.vector_status(&SessionId::new("A")).unwrap(),
        VectorStatus::Ready {
            embedder: "inny/8".into(),
            missing: 1
        }
    );
    let hits = s
        .query(&q("trucizna", Mode::Hybrid), &Caller::Owner)
        .unwrap();
    assert_eq!(hits[0].doc.key, "zly", "dokument bez wektora nadal w FTS");
}
