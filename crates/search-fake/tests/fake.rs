//! Testy atrapy: kontrakt współdzielony + property-based (determinizm, izolacja agentki).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use search_contract::contract_tests::{self, doc};
use search_contract::{Caller, DocKind, Mode, Query, Search, SearchError, SessionId, SessionSet};
use search_fake::{FakeSearch, RecordingIndexer};

#[test]
fn contract_suite() {
    contract_tests::run_all(|| Box::new(FakeSearch::new()));
}

#[test]
fn tx_search_suite_in_memory() {
    let conn = lib_sqlstore::rusqlite::Connection::open_in_memory().unwrap();
    let s = FakeSearch::new();
    contract_tests::tx_search_suite(&s, &s, &conn);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, failure_persistence: None, ..ProptestConfig::default() })]

    /// Wyniki deterministyczne, posortowane, w limicie; agentka nigdy nie widzi cudzej sesji.
    #[test]
    fn deterministic_and_isolated(
        texts in proptest::collection::vec("[a-ząćęłńóśźż ]{1,30}", 1..20),
        q in "[a-ząćęłńóśźż]{1,6}",
        limit in 0_usize..8,
    ) {
        let s = FakeSearch::new();
        for (i, t) in texts.iter().enumerate() {
            let session = if i % 2 == 0 { "A" } else { "B" };
            s.index(&doc(session, DocKind::Turn, &i.to_string(), t)).unwrap();
        }
        for mode in [Mode::Fts, Mode::Vector, Mode::Hybrid] {
            let query = Query { mode, ..Query::in_session(SessionId::new("A"), q.clone(), limit) };
            let agent = Caller::Agent { session: SessionId::new("A") };
            let hits = s.query(&query, &agent).unwrap();
            prop_assert!(hits.len() <= limit);
            prop_assert!(hits.iter().all(|h| h.session == SessionId::new("A")));
            prop_assert!(hits.windows(2).all(|w| w[0].score >= w[1].score));
            prop_assert_eq!(&hits, &s.query(&query, &agent).unwrap());
            let all = Query { sessions: SessionSet::All, ..query };
            let forbidden = matches!(s.query(&all, &agent), Err(SearchError::Forbidden { .. }));
            prop_assert!(forbidden);
        }
    }
}

#[test]
fn recording_indexer_records_and_fails_on_demand() {
    use lib_sqlstore::rusqlite::Connection;
    use search_contract::{DocId, TxIndexer};
    let conn = Connection::open_in_memory().unwrap();
    let rec = RecordingIndexer::new();
    rec.prepare(&conn).unwrap();
    rec.index_in(&conn, &doc("A", DocKind::Turn, "1", "tekst"))
        .unwrap();
    let id = DocId::new(DocKind::Turn, "1");
    rec.remove_in(&conn, &SessionId::new("A"), &id).unwrap();
    assert_eq!(rec.indexed().len(), 1);
    assert_eq!(rec.removed(), vec![id]);
    rec.set_failing(true);
    assert!(
        rec.index_in(&conn, &doc("A", DocKind::Turn, "2", "x"))
            .is_err()
    );
}

/// Embedder asymetryczny: dokument „o X” i zapytanie „pytanie o X” → ten sam kierunek tylko dla
/// zapytań przez `embed_query` (prefiksy jak E5); awaria → dokument bez wektora.
struct Asymmetric {
    fail: std::sync::atomic::AtomicBool,
}

impl search_contract::Embedder for Asymmetric {
    fn model_id(&self) -> &str {
        "asym"
    }
    fn dims(&self) -> usize {
        2
    }
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SearchError::Embedder { reason: "x".into() });
        }
        Ok(texts
            .iter()
            .map(|t| {
                if t.contains("kot") {
                    vec![1.0, 0.0]
                } else {
                    vec![0.0, 1.0]
                }
            })
            .collect())
    }
    fn embed_query(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        // Zapytanie o „zwierzę” celuje w dokumenty o kocie (bez wspólnych słów).
        Ok(texts
            .iter()
            .map(|t| {
                if t.contains("zwierz") {
                    vec![1.0, 0.0]
                } else {
                    vec![0.0, 1.0]
                }
            })
            .collect())
    }
}

#[test]
fn custom_embedder_uses_query_side_and_tolerates_failures() {
    let embedder = std::sync::Arc::new(Asymmetric { fail: false.into() });
    let s = FakeSearch::with_embedder(embedder.clone());
    s.index(&doc("A", DocKind::Memory, "1", "Mruczek to kot"))
        .unwrap();
    s.index(&doc("A", DocKind::Memory, "2", "samochód stoi w garażu"))
        .unwrap();
    embedder
        .fail
        .store(true, std::sync::atomic::Ordering::SeqCst);
    s.index(&doc("A", DocKind::Memory, "3", "drugi kot sąsiada"))
        .unwrap();
    let q = Query {
        mode: Mode::Vector,
        ..Query::in_session(SessionId::new("A"), "jakie mam zwierzę", 1)
    };
    let hits = s.query(&q, &Caller::Owner).unwrap();
    assert_eq!(hits[0].doc.key, "1", "dokument 3 bez wektora poza kNN");
    let all = Query { limit: 10, ..q };
    assert_eq!(s.query(&all, &Caller::Owner).unwrap().len(), 2);
    assert_eq!(s.doc_count(&SessionId::new("A")), 3);
    assert!(format!("{s:?}").contains("asym"));
}
