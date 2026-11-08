//! Budżet (docs/modules/transfer/SPEC.md): eksport 100 MB ≤ 30 s na baseline, strumieniowo.
//! Mierzymy paczkę ~8 MiB i skalujemy budżet liniowo. Ścisły próg tylko przy
//! `ALFA_PERF_BUDGETS=1`; inaczej próg bezpieczeństwa ×10 (crates/README.md).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Instant;

use transfer_contract::contract_tests::{Harness, wipe};
use transfer_contract::{Category, ExportRequest, ExportScope, ImportOptions, Selection, Transfer};

const MIB: usize = 1 << 20;

#[test]
fn export_and_import_within_budget() {
    let h = common::harness();
    // Treść słabo kompresowalna (jak załączniki), 8 × 1 MiB.
    let mut state = 0x9e37_79b9_u32;
    for i in 0..8 {
        let doc: Vec<u8> = (0..MIB)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                b'a' + (state % 26) as u8
            })
            .collect();
        h.store(Category::Logs)
            .write(&format!("log-{i}.ndjson"), &doc)
            .unwrap();
    }
    let pkg = h.path("budzet.alfa");
    let scope = ExportScope {
        logs: true,
        sessions: Selection::All,
        ..ExportScope::default()
    };
    let started = Instant::now();
    let report = h.transfer.export(&ExportRequest::new(scope, &pkg)).unwrap();
    let export_s = started.elapsed().as_secs_f64();
    wipe(&h);
    let started = Instant::now();
    h.transfer.import(&pkg, &ImportOptions::default()).unwrap();
    let import_s = started.elapsed().as_secs_f64();

    let mib = report.manifest.total_bytes() as f64 / MIB as f64;
    let budget_s = 30.0 * mib / 100.0;
    let strict = std::env::var("ALFA_PERF_BUDGETS").is_ok_and(|v| v == "1");
    let limit = if strict { budget_s } else { budget_s * 10.0 };
    eprintln!(
        "transfer: {mib:.1} MiB — eksport {export_s:.2} s, import {import_s:.2} s (budżet {budget_s:.2} s, próg {limit:.2} s, ścisły: {strict})"
    );
    assert!(export_s <= limit, "eksport {export_s:.2} s > {limit:.2} s");
    assert!(import_s <= limit, "import {import_s:.2} s > {limit:.2} s");
}
