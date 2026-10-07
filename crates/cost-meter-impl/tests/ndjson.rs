//! Dziennik NDJSON: zapis/odczyt, urwana linia po awarii, odtworzenie agregatów (property-based).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use cost_meter_contract::contract_tests::{CONTRACT_RATE_E4, contract_day, input};
use cost_meter_contract::{
    BudgetConfig, CostMeter, FxQuote, LedgerStore, Pricing, TotalsQuery, Usage,
};
use cost_meter_fake::{FakeFxSource, FixedClock};
use cost_meter_impl::{CostMeterService, NdjsonLedger};
use proptest::prelude::*;

fn temp_file(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("alfa-cost-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("costs.ndjson")
}

fn meter(path: &PathBuf, clock: Arc<FixedClock>) -> CostMeterService {
    let fx = Arc::new(FakeFxSource::fixed(FxQuote {
        rate_e4: CONTRACT_RATE_E4,
        effective_date: contract_day(),
    }));
    CostMeterService::new(
        Arc::new(NdjsonLedger::new(path)),
        fx,
        clock,
        BudgetConfig::default(),
    )
    .unwrap()
}

#[tokio::test]
async fn torn_line_is_skipped_and_sealed() {
    let path = temp_file("torn");
    let clock = Arc::new(FixedClock::new(contract_day()));
    let m = meter(&path, clock.clone());
    m.refresh_fx().await;
    m.record(input(
        "s",
        "acme",
        Usage::default(),
        Pricing::Reported { micro_usd: 11 },
    ))
    .await
    .unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{\"seq\": 2, \"trunc")
        .unwrap();
    let ledger = NdjsonLedger::new(&path);
    let loaded = ledger.load().unwrap();
    assert_eq!((loaded.records.len(), loaded.skipped_lines), (1, 1));
    let reopened = meter(&path, clock.clone());
    assert_eq!(reopened.skipped_lines(), 1);
    reopened
        .record(input(
            "s",
            "acme",
            Usage::default(),
            Pricing::Reported { micro_usd: 5 },
        ))
        .await
        .unwrap();
    let again = meter(&path, clock);
    assert_eq!(again.totals(&TotalsQuery::All).micro_usd, 16);
    assert_eq!(again.skipped_lines(), 1);
    assert_eq!(
        NdjsonLedger::new(temp_file("missing"))
            .load()
            .unwrap()
            .records
            .len(),
        0
    );
    // Windows nie usunie katalogu z otwartym plikiem — najpierw zamykamy uchwyty.
    drop((m, reopened, again, ledger));
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn reload_reproduces_totals(costs in prop::collection::vec((0u64..5_000_000, 0u8..4, 0i64..40), 1..25)) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let path = temp_file("prop");
        let clock = Arc::new(FixedClock::new(contract_day()));
        let m = meter(&path, clock.clone());
        rt.block_on(m.refresh_fx());
        for (usd, session, day) in &costs {
            clock.set_day(contract_day() + chrono::Duration::days(*day));
            let pricing = if *usd % 7 == 0 { Pricing::Unknown } else { Pricing::Reported { micro_usd: *usd } };
            rt.block_on(m.record(input(&format!("s{session}"), "acme", Usage::default(), pricing))).unwrap();
        }
        let reloaded = meter(&path, clock);
        let a = m.totals(&TotalsQuery::All);
        prop_assert_eq!(a, reloaded.totals(&TotalsQuery::All));
        let entries: u64 = costs.iter().filter(|(u, _, _)| u % 7 != 0).map(|(u, _, _)| *u).sum();
        prop_assert_eq!(a.micro_usd, entries);
        drop((m, reloaded));
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
