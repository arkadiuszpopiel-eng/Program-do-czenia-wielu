//! Testy specyficzne dla SQLite: kaskada w surowych tabelach, szpiegowskie, transakcja wywołującego,
//! zdarzenia modułu (zmiana embeddera i przebudowa: `tests/reindex.rs`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use lib_sqlstore::rusqlite::Connection;
use search_contract::contract_tests::doc;
use search_contract::{Caller, DocId, DocKind, Mode, Query, Search, SessionId, TxIndexer, events};
use search_impl::SqliteSearch;
use sessions_contract::SessionDbProvider;

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn remove_cascades_through_raw_tables() {
    let h = common::harness();
    h.index(&doc("A", DocKind::Memory, "m1", "fakt do zapomnienia"))
        .unwrap();
    h.index(&doc("A", DocKind::Turn, "1", "tura zostaje"))
        .unwrap();
    let db = h.provider.session_db(&SessionId::new("A")).unwrap();
    let before = db
        .with(|c| Ok::<_, ()>(count(c, "SELECT count(*) FROM search_vec_memory")))
        .unwrap();
    assert_eq!(before, 1);
    h.remove(&SessionId::new("A"), &DocId::new(DocKind::Memory, "m1"))
        .unwrap();
    db.with(|c| {
        assert_eq!(
            count(c, "SELECT count(*) FROM search_docs WHERE kind = 'memory'"),
            0
        );
        assert_eq!(count(c, "SELECT count(*) FROM search_vec_memory"), 0);
        assert_eq!(
            count(
                c,
                "SELECT count(*) FROM search_fts WHERE search_fts MATCH 'zapomnienia'"
            ),
            0
        );
        assert_eq!(count(c, "SELECT count(*) FROM search_fts"), 1);
        Ok::<(), ()>(())
    })
    .unwrap();
}

#[test]
fn session_text_never_reaches_other_session_file() {
    let h = common::harness();
    let secret = "szpieg-tekst-sesji-A-9981";
    h.index(&doc("A", DocKind::Turn, "1", secret)).unwrap();
    h.index(&doc("B", DocKind::Turn, "1", "jawne słowo B"))
        .unwrap();
    let q = Query::in_session(SessionId::new("B"), secret, 10);
    assert!(
        h.query(&q, &Caller::Owner)
            .unwrap()
            .iter()
            .all(|hit| hit.session.as_str() == "B")
    );
    let fts_b = Query {
        mode: Mode::Fts,
        ..q
    };
    assert!(h.query(&fts_b, &Caller::Owner).unwrap().is_empty());
    for s in ["A", "B"] {
        let path = h
            .provider
            .session_db(&SessionId::new(s))
            .unwrap()
            .path()
            .to_path_buf();
        for file in lib_sqlstore::database_files(&path) {
            let bytes = std::fs::read(&file).unwrap_or_default();
            assert!(
                !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
                "{s}"
            );
        }
    }
}

#[test]
fn tx_indexer_rolls_back_with_callers_transaction() {
    let h = common::harness();
    let db = h.provider.session_db(&SessionId::new("A")).unwrap();
    db.with(|c| {
        h.search.prepare(c).unwrap();
        let tx = c.transaction().unwrap();
        h.search
            .index_in(&tx, &doc("A", DocKind::Turn, "1", "wycofane słowo"))
            .unwrap();
        drop(tx);
        Ok::<(), ()>(())
    })
    .unwrap();
    let q = Query {
        mode: Mode::Fts,
        ..Query::in_session(SessionId::new("A"), "wycofane", 5)
    };
    assert!(h.query(&q, &Caller::Owner).unwrap().is_empty());
}

#[tokio::test]
async fn module_events_carry_no_content() {
    let mut h = common::harness();
    assert_eq!(h.search.health(), HealthStatus::NotStarted);
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(h.search.manifest().id.clone(), Arc::new(bus.clone()));
    h.search.start(ctx).await.unwrap();
    assert_eq!(h.search.health(), HealthStatus::Healthy);
    h.index(&doc("A", DocKind::Turn, "1", "poufne zapytanie"))
        .unwrap();
    h.query(
        &Query::in_session(SessionId::new("A"), "poufne", 5),
        &Caller::Owner,
    )
    .unwrap();
    h.remove(&SessionId::new("A"), &DocId::new(DocKind::Turn, "1"))
        .unwrap();
    let kind = EventKind::Custom(events::QUERY.to_owned());
    for _ in 0..200 {
        if !bus.recorded_of_kind(&kind).is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let queries = bus.recorded_of_kind(&kind);
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0].payload["hits"], serde_json::json!(1));
    assert!(!queries[0].payload.to_string().contains("poufne"));
    h.search.stop().await.unwrap();
}

#[test]
fn snippet_length_is_configurable() {
    let h = common::harness();
    let short = SqliteSearch::new(
        h.provider.clone(),
        Arc::new(search_fake::HashEmbedder::new()),
    )
    .unwrap()
    .with_snippet_chars(20);
    let long = format!("{} słowo kluczowe {}", "a ".repeat(60), "b ".repeat(60));
    short.index(&doc("A", DocKind::Turn, "1", &long)).unwrap();
    let q = Query {
        mode: Mode::Fts,
        ..Query::in_session(SessionId::new("A"), "kluczowe", 5)
    };
    let hits = short.query(&q, &Caller::Owner).unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].snippet.text.chars().count() <= 22);
    assert_eq!(hits[0].snippet.highlights.len(), 1);
    let unknown = Query::in_session(SessionId::new("A"), "x", 5);
    let many = Query {
        sessions: search_contract::SessionSet::Many(vec![SessionId::new("A"), SessionId::new("A")]),
        ..unknown
    };
    assert!(short.query(&many, &Caller::Owner).is_ok());
}
