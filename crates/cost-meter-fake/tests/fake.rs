//! Testy atrapy: kontrakt współdzielony + sterowany werdykt i atrapy portów.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use accounts_hub_contract::ProviderId;
use cost_meter_contract::contract_tests::{self, CONTRACT_RATE_E4, contract_day, input, price};
use cost_meter_contract::{
    BudgetDecision, CostClock, CostMeter, FxError, FxQuote, FxSource, LedgerStore, Pricing, Usage,
};
use cost_meter_fake::{FakeCostMeter, FakeFxSource, FixedClock, MemoryLedgerStore};

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|| async { FakeCostMeter::new(CONTRACT_RATE_E4, contract_day()) })
        .await;
}

#[tokio::test]
async fn forced_decision_and_spy() {
    let fake = FakeCostMeter::new(CONTRACT_RATE_E4, contract_day());
    fake.force_decision(Some(BudgetDecision::Allow));
    let p = ProviderId::new("acme").unwrap();
    assert_eq!(
        fake.check_budget(u64::MAX, true, Some(&p)).await,
        BudgetDecision::Allow
    );
    fake.force_decision(None);
    assert!(matches!(
        fake.check_budget(1, true, None).await,
        BudgetDecision::Block { .. }
    ));
    assert_eq!(fake.checks().len(), 2);
    assert_eq!(fake.checks()[0].provider, Some(p));
    fake.record(input(
        "s",
        "acme",
        Usage::default(),
        Pricing::Price(price()),
    ))
    .await
    .unwrap();
    fake.set_day(contract_day().succ_opt().unwrap());
    assert_eq!(fake.records().len(), 1);
    assert_eq!(fake.refresh_fx().await.rate_e4, CONTRACT_RATE_E4);
}

#[tokio::test]
async fn port_fakes() {
    let q = FxQuote {
        rate_e4: 36_000,
        effective_date: contract_day(),
    };
    let fx = FakeFxSource::fixed(q);
    fx.push(Err(FxError::Network("x".into())));
    assert!(fx.fetch_usd_pln().await.is_err());
    assert_eq!(fx.fetch_usd_pln().await.unwrap(), q);
    fx.set_default(Err(FxError::Parse("y".into())));
    assert!(fx.fetch_usd_pln().await.is_err());
    assert_eq!(fx.calls(), 3);
    assert!(FakeFxSource::offline().fetch_usd_pln().await.is_err());

    let store = MemoryLedgerStore::new();
    let fake = FakeCostMeter::new(CONTRACT_RATE_E4, contract_day());
    let r = fake
        .record(input("s", "acme", Usage::default(), Pricing::Unknown))
        .await
        .unwrap();
    store.fail_next_append();
    assert!(store.append(&r).is_err());
    store.append(&r).unwrap();
    assert_eq!(store.load().unwrap().records, vec![r.clone()]);
    assert_eq!(MemoryLedgerStore::with_records(vec![r]).records().len(), 1);

    let clock = FixedClock::new(contract_day());
    clock.advance_days(1);
    assert_eq!(clock.today(), contract_day().succ_opt().unwrap());
    clock.set_day(contract_day());
    assert_eq!(clock.now().date_naive(), contract_day());
}
