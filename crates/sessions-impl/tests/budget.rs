//! Budżety wydajności (debug): 1000 tur < 1 s, projekcja 1000 tur < 50 ms (SPEC: `view` ≤ 50 ms).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Instant;

use sessions_contract::{SessionCatalog, SessionHistory};
use sessions_fake::fixtures::linear_conversation;

#[test]
fn append_and_project_1000_turns_within_budget() {
    let h = common::harness();
    let start = Instant::now();
    let (id, turns) = linear_conversation(&*h, 1000).unwrap();
    let append = start.elapsed();
    let leaf = turns.last().unwrap().id;
    // Rozgrzewka (przygotowanie zapytania), potem pomiar najlepszego z 5.
    h.branch_projection(&id, leaf).unwrap();
    let projection = (0..5)
        .map(|_| {
            let t = Instant::now();
            let proj = h.branch_projection(&id, leaf).unwrap();
            assert_eq!(proj.len(), 1000);
            t.elapsed()
        })
        .min()
        .unwrap();
    let t = Instant::now();
    let list = h.list_sessions(&Default::default()).unwrap();
    let listing = t.elapsed();
    assert_eq!(list[0].turns, 1000);
    eprintln!(
        "[budżet sessions] append 1000 tur: {append:?}; projekcja 1000 tur: {projection:?}; lista: {listing:?}"
    );
    assert!(
        append.as_millis() < budget_ms(1000),
        "append 1000 tur: {append:?}"
    );
    assert!(
        projection.as_millis() < budget_ms(50),
        "projekcja: {projection:?}"
    );
}

/// Budżet czasowy: ściśle przy `ALFA_PERF_BUDGETS=1` (maszyna pomiarowa, baseline),
/// na współdzielonym CI tylko próg bezpieczeństwa ×10 (łapie patologiczne regresje).
fn budget_ms(strict_ms: u128) -> u128 {
    if std::env::var_os("ALFA_PERF_BUDGETS").is_some() {
        strict_ms
    } else {
        strict_ms * 10
    }
}
