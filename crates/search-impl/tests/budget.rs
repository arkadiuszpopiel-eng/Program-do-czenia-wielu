//! Budżety (debug): indeksowanie 1000 tur (każda we własnej transakcji) i zapytanie FTS < 20 ms.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::{Duration, Instant};

use search_contract::contract_tests::doc;
use search_contract::{Caller, DocKind, Mode, Query, Search, SessionId};

fn best_of(n: usize, mut f: impl FnMut()) -> Duration {
    f();
    (0..n)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .min()
        .unwrap_or_default()
}

#[test]
fn fts_query_on_1000_turns_within_budget() {
    let h = common::harness();
    let start = Instant::now();
    for i in 0..1000 {
        let text = if i % 50 == 0 {
            format!("Tura {i}: żółta łódź na jeziorze i gęś")
        } else {
            format!("Tura {i}: zwykła rozmowa o pogodzie numer {i}")
        };
        h.index(&doc("A", DocKind::Turn, &i.to_string(), &text))
            .unwrap();
    }
    let indexing = start.elapsed();
    let session = SessionId::new("A");
    let fts = Query {
        mode: Mode::Fts,
        ..Query::in_session(session.clone(), "zolta lodz", 20)
    };
    assert_eq!(h.query(&fts, &Caller::Owner).unwrap().len(), 20);
    let fts_time = best_of(5, || {
        h.query(&fts, &Caller::Owner).unwrap();
    });
    let hybrid = Query::in_session(session.clone(), "żółta łódź", 20);
    let hybrid_time = best_of(5, || {
        h.query(&hybrid, &Caller::Owner).unwrap();
    });
    let vector = Query {
        mode: Mode::Vector,
        ..Query::in_session(session, "pogoda", 20)
    };
    let vector_time = best_of(5, || {
        h.query(&vector, &Caller::Owner).unwrap();
    });
    eprintln!(
        "[budżet search] indeksowanie 1000 tur: {indexing:?}; FTS: {fts_time:?}; wektor: {vector_time:?}; hybryda: {hybrid_time:?}"
    );
    assert!(fts_time.as_millis() < 20, "FTS {fts_time:?}");
}
