//! Testy implementacji: kontrakt, kurs NBP (cache dzienny, brak sieci), alerty, zdarzenia,
//! trwałość NDJSON, błędy zapisu, cykl życia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};
use cost_meter_contract::contract_tests::{self, CONTRACT_RATE_E4, contract_day, input, price};
use cost_meter_contract::{
    BudgetConfig, BudgetDecision, BudgetOrigin, CostError, CostMeter, EVENT_COST_RECORDED,
    EVENT_FX_STALE, EVENT_FX_UPDATED, EVENT_LIMIT_BLOCKED, EVENT_LIMIT_WARNING, FxError, FxOrigin,
    FxQuote, FxSource, LimitMode, MonthlyLimit, Pricing, TotalsQuery, Usage, event_kind,
};
use cost_meter_fake::{FakeFxSource, FixedClock, MemoryLedgerStore};
use cost_meter_impl::{CostMeterService, HttpGet, MODULE_TOML, NbpFxSource};

fn quote() -> FxQuote {
    FxQuote {
        rate_e4: CONTRACT_RATE_E4,
        effective_date: contract_day(),
    }
}

struct Parts {
    store: Arc<MemoryLedgerStore>,
    fx: Arc<FakeFxSource>,
    clock: Arc<FixedClock>,
}

fn parts(fx: FakeFxSource) -> Parts {
    Parts {
        store: Arc::new(MemoryLedgerStore::new()),
        fx: Arc::new(fx),
        clock: Arc::new(FixedClock::new(contract_day())),
    }
}

fn service(p: &Parts, budget: BudgetConfig) -> CostMeterService {
    CostMeterService::new(p.store.clone(), p.fx.clone(), p.clock.clone(), budget).unwrap()
}

async fn started(p: &Parts, budget: BudgetConfig) -> (CostMeterService, FakeBus) {
    let mut s = service(p, budget);
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(s.manifest().id.clone(), Arc::new(bus.clone()));
    s.start(ctx).await.unwrap();
    (s, bus)
}

fn reported(micro_usd: u64) -> cost_meter_contract::CostInput {
    input(
        "s",
        "acme",
        Usage::default(),
        Pricing::Reported { micro_usd },
    )
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|| async {
        let s = service(
            &parts(FakeFxSource::fixed(quote())),
            BudgetConfig::default(),
        );
        s.refresh_fx().await;
        s
    })
    .await;
}

#[tokio::test]
async fn offline_fx_uses_fallback_and_never_blocks() {
    let p = parts(FakeFxSource::offline());
    let (s, bus) = started(&p, BudgetConfig::default()).await;
    let rate = s.refresh_fx().await;
    assert_eq!((rate.rate_e4, rate.origin), (40_000, FxOrigin::Fallback));
    assert!(rate.stale);
    assert_eq!(bus.recorded_of_kind(&event_kind(EVENT_FX_STALE)).len(), 1);
    assert!(matches!(s.health(), HealthStatus::Degraded(_)));
    assert_eq!(
        s.check_budget(10_000, false, None).await,
        BudgetDecision::Allow
    );
    let r = s.record(reported(1_000_000)).await.unwrap();
    assert_eq!(r.micro_pln, Some(4_000_000));
    assert_eq!(r.fx.origin, FxOrigin::Fallback);
}

#[tokio::test]
async fn fx_refreshed_once_per_day_and_previous_rate_kept_on_failure() {
    let p = parts(FakeFxSource::fixed(quote()));
    let (s, bus) = started(&p, BudgetConfig::default()).await;
    s.refresh_fx().await;
    s.refresh_fx().await;
    assert_eq!(p.fx.calls(), 1);
    assert_eq!(s.health(), HealthStatus::Healthy);
    p.clock.advance_days(1);
    p.fx.push(Err(FxError::Network("503".into())));
    let rate = s.refresh_fx().await;
    assert_eq!(
        (rate.rate_e4, rate.origin, rate.stale),
        (CONTRACT_RATE_E4, FxOrigin::Nbp, true)
    );
    assert_eq!(p.fx.calls(), 2);
    assert_eq!(bus.recorded_of_kind(&event_kind(EVENT_FX_UPDATED)).len(), 1);
    assert_eq!(bus.recorded_of_kind(&event_kind(EVENT_FX_STALE)).len(), 1);
    assert!(!s.refresh_fx().await.stale);
}

#[tokio::test]
async fn threshold_alerts_fire_once_and_block_is_announced() {
    let p = parts(FakeFxSource::fixed(quote()));
    let budget = BudgetConfig {
        monthly: MonthlyLimit::pln(1, LimitMode::Enforced),
        ..BudgetConfig::default()
    };
    let (s, bus) = started(&p, budget).await;
    s.refresh_fx().await;
    // 1 PLN = 1 000 000 mikro-PLN; 150 000 mikro-USD × 3,6512 ≈ 547 680 (55%).
    s.record(reported(150_000)).await.unwrap();
    s.record(reported(10)).await.unwrap();
    s.record(reported(150_000)).await.unwrap();
    let warnings = bus.recorded_of_kind(&event_kind(EVENT_LIMIT_WARNING));
    let thresholds: Vec<u64> = warnings
        .iter()
        .map(|e| e.payload["threshold_pct"].as_u64().unwrap())
        .collect();
    assert_eq!(thresholds, vec![50, 80, 100]);
    assert_eq!(
        bus.recorded_of_kind(&event_kind(EVENT_COST_RECORDED)).len(),
        3
    );
    assert!(matches!(
        s.check_budget(1, false, None).await,
        BudgetDecision::Block { .. }
    ));
    let blocked = bus.recorded_of_kind(&event_kind(EVENT_LIMIT_BLOCKED));
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].payload["scope"]["scope"], "monthly");
}

#[tokio::test]
async fn storage_failure_does_not_change_totals() {
    let p = parts(FakeFxSource::fixed(quote()));
    let s = service(&p, BudgetConfig::default());
    s.record(reported(5)).await.unwrap();
    p.store.fail_next_append();
    assert!(matches!(
        s.record(reported(7)).await,
        Err(CostError::Storage(_))
    ));
    assert_eq!(s.totals(&TotalsQuery::All).micro_usd, 5);
    assert_eq!(s.record(reported(7)).await.unwrap().seq, 2);
}

#[tokio::test]
async fn restart_restores_totals_and_rate() {
    let p = parts(FakeFxSource::fixed(quote()));
    let s = service(&p, BudgetConfig::default());
    s.refresh_fx().await;
    s.record(input(
        "s1",
        "acme",
        Usage {
            input_tokens: 1_000,
            ..Usage::default()
        },
        Pricing::Price(price()),
    ))
    .await
    .unwrap();
    s.record(reported(9)).await.unwrap();
    let offline = Arc::new(FakeFxSource::offline());
    let restarted = CostMeterService::new(
        p.store.clone(),
        offline,
        p.clock.clone(),
        BudgetConfig::default(),
    )
    .unwrap();
    assert_eq!(
        restarted.totals(&TotalsQuery::All),
        s.totals(&TotalsQuery::All)
    );
    let rate = restarted.current_rate();
    assert_eq!(
        (rate.rate_e4, rate.origin, rate.stale),
        (CONTRACT_RATE_E4, FxOrigin::Nbp, false)
    );
    assert_eq!(restarted.record(reported(1)).await.unwrap().seq, 3);
    assert_eq!(restarted.skipped_lines(), 0);
}

#[tokio::test]
async fn lifecycle() {
    let p = parts(FakeFxSource::fixed(quote()));
    let mut s = service(&p, BudgetConfig::default());
    assert_eq!(s.health(), HealthStatus::NotStarted);
    assert_eq!(s.stop().await, Err(ModuleError::NotStarted));
    let ctx = ModuleContext::new(s.manifest().id.clone(), Arc::new(FakeBus::default()));
    s.start(ctx.clone()).await.unwrap();
    assert_eq!(s.start(ctx).await, Err(ModuleError::AlreadyStarted));
    s.stop().await.unwrap();
    assert_eq!(s.manifest().id.as_str(), "cost-meter");
    assert!(MODULE_TOML.contains("net.egress(api.nbp.pl)"));
    let bad = BudgetConfig {
        warn_at_pct: 0,
        ..BudgetConfig::default()
    };
    assert!(CostMeterService::new(p.store.clone(), p.fx.clone(), p.clock.clone(), bad).is_err());
    assert!(
        s.set_budget(BudgetConfig::default(), BudgetOrigin::User)
            .await
            .is_ok()
    );
}

struct Http(Result<String, String>, u64);

#[async_trait]
impl HttpGet for Http {
    async fn get_text(&self, url: &str) -> Result<String, String> {
        assert!(url.starts_with("https://api.nbp.pl/api/exchangerates/rates/a/usd/"));
        tokio::time::sleep(std::time::Duration::from_millis(self.1)).await;
        self.0.clone()
    }
}

#[tokio::test]
async fn nbp_source_parses_and_times_out() {
    let body = r#"{"table":"A","code":"USD","rates":[{"no":"1/A/NBP/2026","effectiveDate":"2026-09-30","mid":3.6512}]}"#;
    let ok = NbpFxSource::new(Http(Ok(body.into()), 0));
    assert_eq!(ok.fetch_usd_pln().await.unwrap(), quote());
    let down = NbpFxSource::new(Http(Err("HTTP 503".into()), 0));
    assert_eq!(
        down.fetch_usd_pln().await,
        Err(FxError::Network("HTTP 503".into()))
    );
    let slow = NbpFxSource::new(Http(Ok(body.into()), 5_000))
        .with_timeout(std::time::Duration::from_millis(20));
    assert!(matches!(
        slow.fetch_usd_pln().await,
        Err(FxError::Network(_))
    ));
    let garbage = NbpFxSource::new(Http(Ok("<html>".into()), 0));
    assert!(matches!(
        garbage.fetch_usd_pln().await,
        Err(FxError::Parse(_))
    ));
}

#[tokio::test]
async fn record_is_fast_enough() {
    let p = parts(FakeFxSource::fixed(quote()));
    let s = service(&p, BudgetConfig::default());
    let started = std::time::Instant::now();
    for _ in 0..2_000 {
        s.record(reported(3)).await.unwrap();
    }
    // Budżet SPEC: record ≤ 0,5 ms; tu luźny próg dla buildu debug w CI.
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(s.totals(&TotalsQuery::All).calls, 2_000);
}
