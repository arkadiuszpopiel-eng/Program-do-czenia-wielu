//! Property-based (ACC-F1-cost-meter-01/02): suma agregatów = suma wpisów; PLN zgodne z kursem
//! z rekordu; wyłączony limit nigdy nie blokuje; progi alertów przekraczane raz.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use accounts_hub_contract::{CostLimit, ModelId, ModelPrice, ProviderId};
use chrono::{NaiveDate, TimeZone, Utc};
use core_bus_contract::SessionId;
use cost_meter_contract::{
    BudgetConfig, BudgetDecision, CostInput, CostRecord, FxOrigin, FxRate, Ledger, LimitMode,
    Month, MonthlyLimit, Pricing, Spent, Totals, TotalsQuery, Usage, crossed_thresholds, evaluate,
    usd_to_pln,
};
use proptest::prelude::*;

#[derive(Debug, Clone)]
struct Gen {
    session: u8,
    provider: u8,
    day_offset: u16,
    usage: (u32, u32, u32),
    pricing: u8,
    rate_e4: u32,
    background: bool,
}

fn gen_record() -> impl Strategy<Value = Gen> {
    (
        0u8..5,
        0u8..4,
        0u16..120,
        (0u32..2_000_000, 0u32..200_000, 0u32..1_000_000),
        0u8..3,
        30_000u32..45_000,
        any::<bool>(),
    )
        .prop_map(
            |(session, provider, day_offset, usage, pricing, rate_e4, background)| Gen {
                session,
                provider,
                day_offset,
                usage,
                pricing,
                rate_e4,
                background,
            },
        )
}

fn record(i: usize, g: &Gen) -> CostRecord {
    let day = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
        + chrono::Duration::days(i64::from(g.day_offset));
    let pricing = match g.pricing {
        0 => Pricing::Price(ModelPrice {
            input_micro_usd_per_mtok: 3_000_000,
            output_micro_usd_per_mtok: 15_000_000,
            cache_read_micro_usd_per_mtok: 300_000,
            cache_write_micro_usd_per_mtok: 0,
        }),
        1 => Pricing::Reported {
            micro_usd: u64::from(g.usage.0),
        },
        _ => Pricing::Unknown,
    };
    let input = CostInput {
        session: Some(SessionId::new(format!("s{}", g.session))),
        agent: None,
        provider: ProviderId::new(format!("p{}", g.provider)).unwrap(),
        account: None,
        model: ModelId::new("m").unwrap(),
        usage: Usage {
            input_tokens: u64::from(g.usage.0),
            output_tokens: u64::from(g.usage.1),
            cache_read_tokens: u64::from(g.usage.2),
            cache_write_tokens: 0,
        },
        pricing,
        background: g.background,
    };
    let fx = FxRate {
        rate_e4: g.rate_e4,
        effective_date: Some(day),
        origin: FxOrigin::Nbp,
        stale: false,
    };
    let ts = Utc.from_utc_datetime(&day.and_hms_opt(12, 0, 0).unwrap());
    CostRecord::from_input(input, i as u64 + 1, ts, day, fx)
}

fn sum_of(parts: impl Iterator<Item = Totals>) -> (u64, u64, u64, u64) {
    parts.fold((0, 0, 0, 0), |acc, t| {
        (
            acc.0 + t.calls,
            acc.1 + t.micro_usd,
            acc.2 + t.micro_pln,
            acc.3 + t.unknown_cost_calls,
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn aggregate_sums_equal_entry_sums(gens in prop::collection::vec(gen_record(), 0..60)) {
        let records: Vec<CostRecord> = gens.iter().enumerate().map(|(i, g)| record(i, g)).collect();
        let ledger = Ledger::from_records(records.clone());
        let entries = (
            records.len() as u64,
            records.iter().filter_map(|r| r.micro_usd).sum::<u64>(),
            records.iter().filter_map(|r| r.micro_pln).sum::<u64>(),
            records.iter().filter(|r| r.micro_usd.is_none()).count() as u64,
        );
        let all = ledger.totals(&TotalsQuery::All);
        prop_assert_eq!((all.calls, all.micro_usd, all.micro_pln, all.unknown_cost_calls), entries);
        let by_session = sum_of(ledger.sessions().into_iter().map(|session| ledger.totals(&TotalsQuery::Session { session })));
        let by_day = sum_of(ledger.days().into_iter().map(|day| ledger.totals(&TotalsQuery::Day { day })));
        let by_month = sum_of(ledger.months().into_iter().map(|month| ledger.totals(&TotalsQuery::Month { month })));
        let by_provider = sum_of(ledger.provider_months().into_iter().map(|(provider, month)| ledger.totals(&TotalsQuery::Provider { provider, month })));
        for sums in [by_session, by_day, by_month, by_provider] {
            prop_assert_eq!(sums, entries);
        }
        for r in &records {
            if let (Some(usd), Some(pln)) = (r.micro_usd, r.micro_pln) {
                prop_assert_eq!(pln, usd_to_pln(usd, r.fx.rate_e4));
            }
        }
        let bg: u64 = records.iter().filter(|r| r.background).filter_map(|r| r.micro_usd).sum();
        let bg_ledger: u64 = ledger.months().into_iter().map(|month| ledger.totals(&TotalsQuery::Background { month }).micro_usd).sum();
        prop_assert_eq!(bg, bg_ledger);
    }

    #[test]
    fn disabled_limits_never_block(
        spent in any::<u64>(), bg in any::<u64>(), prov in any::<u64>(), est in any::<u64>(),
        limit in any::<u32>(), mode in 0u8..2, background in any::<bool>(),
    ) {
        let mode = if mode == 0 { LimitMode::AlertOnly } else { LimitMode::Off };
        let p = ProviderId::new("p").unwrap();
        let cfg = BudgetConfig {
            monthly: MonthlyLimit { amount_micro_pln: u64::from(limit), mode },
            background: MonthlyLimit { amount_micro_pln: u64::from(limit), mode },
            providers: [(p.clone(), CostLimit { enabled: false, monthly_limit_grosze: u64::from(limit) })].into(),
            ..BudgetConfig::default()
        };
        let spent = Spent { month_micro_pln: spent, background_micro_pln: bg, provider_micro_pln: prov };
        let d = evaluate(&cfg, spent, est, background, Some(&p));
        let blocked = matches!(d, BudgetDecision::Block { .. });
        prop_assert!(!blocked);
    }

    #[test]
    fn enforced_blocks_iff_over_limit(spent in 0u64..10_000_000, est in 0u64..10_000_000, limit in 0u64..10_000_000) {
        let cfg = BudgetConfig {
            monthly: MonthlyLimit { amount_micro_pln: limit, mode: LimitMode::Enforced },
            ..BudgetConfig::default()
        };
        let d = evaluate(&cfg, Spent { month_micro_pln: spent, ..Spent::default() }, est, false, None);
        let blocked = matches!(d, BudgetDecision::Block { .. });
        prop_assert_eq!(blocked, spent + est > limit);
    }

    #[test]
    fn thresholds_crossed_once(steps in prop::collection::vec(0u64..400_000, 1..30)) {
        let limit = 1_000_000;
        let mut spent = 0u64;
        let mut seen = Vec::new();
        for s in steps {
            let after = spent + s;
            seen.extend(crossed_thresholds(spent, after, limit, &[50, 80, 100]));
            spent = after;
        }
        let mut dedup = seen.clone();
        dedup.dedup();
        prop_assert_eq!(&seen, &dedup);
        let reached = (spent * 100 / limit) as u32;
        let expected: Vec<u8> = [50u8, 80, 100].into_iter().filter(|t| u32::from(*t) <= reached).collect();
        prop_assert_eq!(seen, expected);
    }
}

#[test]
fn month_of_day() {
    let m = Month::of(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
    assert_eq!((m.year, m.month), (2026, 12));
}
