//! Współdzielone testy kontraktowe (feature `contract-tests`), `ACC-F1-search-01..03`.
//! Ten sam zestaw uruchamiają `search-impl` i `search-fake` (z tym samym embedderem atrapy).

use std::ops::Deref;

use chrono::{DateTime, Utc};
use core_bus_contract::SessionId;

use crate::api::Search;
use crate::error::SearchError;
use crate::types::{Caller, Doc, DocId, DocKind, Hit, Mode, Query, SessionSet};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("nieoczekiwany błąd: {e}"))
}

fn sid(s: &str) -> SessionId {
    SessionId::new(s)
}

/// Dokument testowy.
pub fn doc(session: &str, kind: DocKind, key: &str, text: &str) -> Doc {
    Doc {
        id: DocId::new(kind, key),
        session: sid(session),
        text: text.into(),
        ts: DateTime::<Utc>::default(),
    }
}

fn query(session: &str, text: &str, mode: Mode) -> Query {
    Query {
        mode,
        ..Query::in_session(sid(session), text, 10)
    }
}

fn keys(hits: &[Hit]) -> Vec<String> {
    hits.iter().map(|h| h.doc.key.clone()).collect()
}

fn seed(s: &dyn Search) {
    for (key, text) in [
        ("1", "Kolor żółć i gęś jaźń"),
        ("2", "Żółta łódź płynie po jeziorze"),
        ("3", "Kot siedzi na macie"),
        ("4", "Pies biega po parku, a kot śpi"),
    ] {
        ok(s.index(&doc("A", DocKind::Turn, key, text)));
    }
}

/// FTS bez diakrytyków, prefiksy, AND; podświetlenie w oryginale.
pub fn fts_folds_and_highlights(s: &dyn Search) {
    seed(s);
    for q in ["zolc", "ŻÓŁĆ", "żółć"] {
        let hits = ok(s.query(&query("A", q, Mode::Fts), &Caller::Owner));
        assert_eq!(keys(&hits), vec!["1"], "zapytanie {q}");
        let h = hits[0].snippet.highlights[0];
        let marked: String = hits[0]
            .snippet
            .text
            .chars()
            .skip(h.start)
            .take(h.end - h.start)
            .collect();
        assert_eq!(marked, "żółć");
    }
    let prefix = ok(s.query(&query("A", "łód pły", Mode::Fts), &Caller::Owner));
    assert_eq!(keys(&prefix), vec!["2"]);
    let and = ok(s.query(&query("A", "łódź pies", Mode::Fts), &Caller::Owner));
    assert!(and.is_empty());
    let kot = ok(s.query(&query("A", "kot", Mode::Fts), &Caller::Owner));
    let mut kot_keys = keys(&kot);
    kot_keys.sort();
    assert_eq!(kot_keys, vec!["3", "4"]);
    assert!(ok(s.query(&query("A", "  ;; ", Mode::Fts), &Caller::Owner)).is_empty());
    assert!(ok(s.query(&query("A", "OR \"x NEAR(", Mode::Fts), &Caller::Owner)).is_empty());
}

/// Wektor: identyczny tekst jest najbliższy; hybryda premiuje zgodność obu list.
pub fn vector_and_hybrid(s: &dyn Search) {
    seed(s);
    let v = ok(s.query(
        &query("A", "Kot siedzi na macie", Mode::Vector),
        &Caller::Owner,
    ));
    assert_eq!(v.first().map(|h| h.doc.key.as_str()), Some("3"));
    assert!(v.windows(2).all(|w| w[0].score >= w[1].score));
    let h = ok(s.query(&query("A", "żółta łódź", Mode::Hybrid), &Caller::Owner));
    assert_eq!(h.first().map(|h| h.doc.key.as_str()), Some("2"));
}

/// Filtr rodzajów i limit; wyniki deterministyczne.
pub fn kinds_limit_determinism(s: &dyn Search) {
    for i in 0..30 {
        ok(s.index(&doc(
            "A",
            DocKind::Turn,
            &format!("t{i:02}"),
            &format!("notatka numer {i}"),
        )));
    }
    ok(s.index(&doc("A", DocKind::Memory, "m1", "notatka w pamięci")));
    let mut q = query("A", "notatka", Mode::Fts);
    q.kinds = vec![DocKind::Memory];
    assert_eq!(keys(&ok(s.query(&q, &Caller::Owner))), vec!["m1"]);
    q.kinds = vec![];
    q.limit = 5;
    let first = ok(s.query(&q, &Caller::Owner));
    assert_eq!(first.len(), 5);
    assert_eq!(first, ok(s.query(&q, &Caller::Owner)));
    q.limit = 0;
    assert!(ok(s.query(&q, &Caller::Owner)).is_empty());
    q.mode = Mode::Vector;
    q.limit = 3;
    q.kinds = vec![DocKind::Memory];
    assert_eq!(keys(&ok(s.query(&q, &Caller::Owner))), vec!["m1"]);
}

/// Ponowne `index` tego samego `DocId` zastępuje treść.
pub fn reindex_replaces(s: &dyn Search) {
    ok(s.index(&doc("A", DocKind::Turn, "1", "stara treść o jabłkach")));
    ok(s.index(&doc("A", DocKind::Turn, "1", "nowa treść o gruszkach")));
    assert!(ok(s.query(&query("A", "jabłkach", Mode::Fts), &Caller::Owner)).is_empty());
    assert_eq!(
        keys(&ok(s.query(
            &query("A", "gruszkach", Mode::Hybrid),
            &Caller::Owner
        ))),
        vec!["1"]
    );
}

/// Kaskada usunięcia: 0 trafień w każdym trybie (`ACC-F1-search-03`).
pub fn remove_cascades(s: &dyn Search) {
    seed(s);
    let id = DocId::new(DocKind::Turn, "3");
    let report = ok(s.remove(&sid("A"), &id));
    assert_eq!((report.docs, report.fts_rows, report.vectors), (1, 1, 1));
    for mode in [Mode::Fts, Mode::Vector, Mode::Hybrid] {
        let hits = ok(s.query(&query("A", "Kot siedzi na macie", mode), &Caller::Owner));
        assert!(!hits.iter().any(|h| h.doc == id), "tryb {mode:?}");
    }
    assert_eq!(ok(s.remove(&sid("A"), &id)).docs, 0);
}

/// Izolacja: agentka widzi tylko własną sesję (`ACC-F1-search-02`, 0/1000 prób).
pub fn agent_isolation(s: &dyn Search) {
    ok(s.index(&doc("A", DocKind::Turn, "a1", "sekret alfa wspólne słowo")));
    ok(s.index(&doc("B", DocKind::Turn, "b1", "sekret beta wspólne słowo")));
    let agent_a = Caller::Agent { session: sid("A") };
    for i in 0..1000 {
        let target = if i % 2 == 0 { "B" } else { "A" };
        let q = query(target, "sekret wspólne", Mode::Fts);
        match s.query(&q, &agent_a) {
            Ok(hits) => assert!(hits.iter().all(|h| h.session == sid("A")) && target == "A"),
            Err(e) => assert!(matches!(e, SearchError::Forbidden { .. }) && target == "B"),
        }
    }
    let mut all = query("A", "sekret", Mode::Fts);
    all.sessions = SessionSet::All;
    assert!(matches!(
        s.query(&all, &agent_a),
        Err(SearchError::Forbidden { .. })
    ));
    let owner = ok(s.query(&all, &Caller::Owner));
    let mut sessions: Vec<String> = owner.iter().map(|h| h.session.to_string()).collect();
    sessions.sort();
    assert_eq!(sessions, vec!["A", "B"]);
    all.sessions = SessionSet::Many(vec![sid("B")]);
    assert_eq!(keys(&ok(s.query(&all, &Caller::Owner))), vec!["b1"]);
}

/// Uruchamia cały zestaw; `factory` daje świeżą, pustą instancję.
pub fn run_all<H, S>(factory: impl Fn() -> H)
where
    H: Deref<Target = S>,
    S: Search,
{
    let cases: [fn(&dyn Search); 6] = [
        fts_folds_and_highlights,
        vector_and_hybrid,
        kinds_limit_determinism,
        reindex_replaces,
        remove_cascades,
        agent_isolation,
    ];
    for case in cases {
        let harness = factory();
        case(&*harness);
    }
}

/// `TxSearcher` + `TxIndexer` w połączeniu wywołującego (baza zakresu pamięci): FTS „dowolne słowo”
/// (OR) i „wszystkie słowa” (AND), osobny tekst embeddingu, filtr rodzajów, etykieta bazy,
/// usunięcie i zatarcie (`compact_in`). `conn` — świeża baza (w `-impl` szyfrowana z sqlite-vec).
pub fn tx_search_suite(
    indexer: &dyn crate::api::TxIndexer,
    searcher: &dyn crate::api::TxSearcher,
    conn: &lib_sqlstore::rusqlite::Connection,
) {
    use crate::types::ConnQuery;
    let label = sid("@global");
    ok(indexer.prepare(conn));
    for (key, text) in [
        ("m1", "Herbata zielona bez cukru"),
        ("m2", "Kawa czarna z cukrem"),
        ("m3", "Rower stoi w garażu"),
    ] {
        ok(indexer.index_in(conn, &doc("@global", DocKind::Memory, key, text)));
    }
    ok(indexer.index_in(
        conn,
        &doc("@global", DocKind::Turn, "t1", "Herbata w turze"),
    ));
    let mut q = ConnQuery::hybrid("herbat kaw", 10, vec![DocKind::Memory]);
    q.mode = Mode::Fts;
    assert!(ok(searcher.query_in(conn, &label, &q)).is_empty(), "AND");
    q.match_any = true;
    let mut any = keys(&ok(searcher.query_in(conn, &label, &q)));
    any.sort();
    assert_eq!(any, vec!["m1", "m2"], "OR");
    q.kinds = vec![];
    assert_eq!(ok(searcher.query_in(conn, &label, &q)).len(), 3);
    let mut v = ConnQuery::hybrid("xyz", 2, vec![DocKind::Memory]);
    v.mode = Mode::Vector;
    v.vector_text = Some("Rower stoi w garażu".into());
    let hits = ok(searcher.query_in(conn, &label, &v));
    assert_eq!(hits.first().map(|h| h.doc.key.as_str()), Some("m3"));
    assert!(hits.iter().all(|h| h.session == label) && hits.len() <= 2);
    let mut h = ConnQuery::hybrid("rower garaz", 5, vec![DocKind::Memory]);
    h.match_any = true;
    assert_eq!(
        ok(searcher.query_in(conn, &label, &h))
            .first()
            .map(|h| h.doc.key.clone()),
        Some("m3".into())
    );
    let removed = ok(indexer.remove_in(conn, &label, &DocId::new(DocKind::Memory, "m3")));
    assert_eq!((removed.docs, removed.fts_rows, removed.vectors), (1, 1, 1));
    ok(indexer.compact_in(conn));
    for mode in [Mode::Fts, Mode::Vector, Mode::Hybrid] {
        h.mode = mode;
        let hits = ok(searcher.query_in(conn, &label, &h));
        assert!(hits.iter().all(|x| x.doc.key != "m3"), "tryb {mode:?}");
    }
    h.limit = 0;
    assert!(ok(searcher.query_in(conn, &label, &h)).is_empty());
}
