//! Współdzielone testy kontraktowe (feature `contract-tests`), uruchamiane na `-impl` i `-fake`.
//! Hub pod testem musi mieć kurs [`CONTRACT_RATE_E4`] (NBP) i dzień [`contract_day`].

use std::collections::BTreeMap;
use std::future::Future;

use accounts_hub_contract::{CostLimit, ModelId, ModelPrice, ProviderId};
use chrono::NaiveDate;
use core_bus_contract::SessionId;

use crate::{
    BudgetConfig, BudgetDecision, BudgetOrigin, BudgetScope, CostError, CostInput, CostMeter,
    FxOrigin, LimitMode, Month, MonthlyLimit, Pricing, TotalsQuery, Usage, cost_micro_usd,
    usd_to_pln,
};

/// Kurs używany w testach kontraktowych (3,6512 PLN/USD).
pub const CONTRACT_RATE_E4: u32 = 36_512;

/// Dzień testów kontraktowych.
pub fn contract_day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 30).unwrap_or_default()
}

/// Cena przykładowa: 3 / 15 USD za Mtok, cache 0,30 USD.
pub fn price() -> ModelPrice {
    ModelPrice {
        input_micro_usd_per_mtok: 3_000_000,
        output_micro_usd_per_mtok: 15_000_000,
        cache_read_micro_usd_per_mtok: 300_000,
        cache_write_micro_usd_per_mtok: 3_750_000,
    }
}

fn pid(s: &str) -> ProviderId {
    ProviderId::new(s).unwrap_or_else(|e| panic!("{e}"))
}

/// Wejście kosztu dla sesji/dostawcy.
pub fn input(session: &str, provider: &str, usage: Usage, pricing: Pricing) -> CostInput {
    CostInput {
        session: Some(SessionId::new(session)),
        agent: None,
        provider: pid(provider),
        account: None,
        model: ModelId::new("model-x").unwrap_or_else(|e| panic!("{e}")),
        usage,
        pricing,
        background: false,
    }
}

fn usage(i: u64, o: u64) -> Usage {
    Usage {
        input_tokens: i,
        output_tokens: o,
        ..Usage::default()
    }
}

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// Przeliczenie i agregaty: sesja, dzień, miesiąc, dostawca; koszt nieznany liczony osobno.
pub async fn record_converts_and_aggregates<C: CostMeter>(c: &C) {
    assert_eq!(c.current_rate().rate_e4, CONTRACT_RATE_E4);
    assert_eq!(c.current_rate().origin, FxOrigin::Nbp);
    let u = usage(10_000, 2_000);
    let a = ok(c
        .record(input("s1", "acme", u, Pricing::Price(price())))
        .await);
    let expected_usd = cost_micro_usd(&u, &price());
    assert_eq!(a.micro_usd, Some(expected_usd));
    assert_eq!(
        a.micro_pln,
        Some(usd_to_pln(expected_usd, CONTRACT_RATE_E4))
    );
    assert_eq!(a.day, contract_day());
    let b = ok(c
        .record(input("s2", "beta", u, Pricing::Reported { micro_usd: 7 }))
        .await);
    let unknown = ok(c.record(input("s1", "beta", u, Pricing::Unknown)).await);
    assert_eq!(unknown.micro_usd, None);
    assert!(b.seq > a.seq && unknown.seq > b.seq);
    let all = c.totals(&TotalsQuery::All);
    assert_eq!(all.calls, 3);
    assert_eq!(all.unknown_cost_calls, 1);
    assert_eq!(all.micro_usd, expected_usd + 7);
    assert_eq!(all.input_tokens, 30_000);
    let s1 = c.totals(&TotalsQuery::Session {
        session: SessionId::new("s1"),
    });
    assert_eq!((s1.calls, s1.micro_usd), (2, expected_usd));
    let month = Month::of(contract_day());
    assert_eq!(c.totals(&TotalsQuery::Month { month }), all);
    assert_eq!(
        c.totals(&TotalsQuery::Day {
            day: contract_day()
        }),
        all
    );
    let beta = c.totals(&TotalsQuery::Provider {
        provider: pid("beta"),
        month,
    });
    assert_eq!((beta.calls, beta.micro_usd), (2, 7));
    let est = c.estimate(&u, &price());
    assert_eq!(est.micro_usd, expected_usd);
    assert_eq!(est.micro_pln, a.micro_pln.unwrap_or_default());
}

/// Limit włączony: ostrzeżenie przy ≥ 80%, blokada po przekroczeniu.
pub async fn enforced_limit_blocks<C: CostMeter>(c: &C) {
    let cfg = BudgetConfig {
        monthly: MonthlyLimit::pln(1, LimitMode::Enforced),
        ..BudgetConfig::default()
    };
    ok(c.set_budget(cfg, BudgetOrigin::User).await);
    assert_eq!(
        c.check_budget(100_000, false, None).await,
        BudgetDecision::Allow
    );
    ok(c.record(input(
        "s",
        "acme",
        Usage::default(),
        Pricing::Reported { micro_usd: 230_000 },
    ))
    .await);
    let warn = c.check_budget(50_000, false, None).await;
    assert!(matches!(warn, BudgetDecision::Warn { .. }), "{warn:?}");
    match c.check_budget(200_000, false, None).await {
        BudgetDecision::Block { notice } => assert_eq!(notice.scope, BudgetScope::Monthly),
        other => panic!("{other:?}"),
    }
}

/// Limit wyłączony: nigdy blokady (AlertOnly → ostrzeżenia, Off → nic).
pub async fn disabled_limit_never_blocks<C: CostMeter>(c: &C) {
    for mode in [LimitMode::AlertOnly, LimitMode::Off] {
        let cfg = BudgetConfig {
            monthly: MonthlyLimit::pln(1, mode),
            ..BudgetConfig::default()
        };
        ok(c.set_budget(cfg, BudgetOrigin::User).await);
        ok(c.record(input(
            "s",
            "acme",
            Usage::default(),
            Pricing::Reported {
                micro_usd: 10_000_000,
            },
        ))
        .await);
        for est in [0, 1, u64::MAX / 4] {
            let d = c.check_budget(est, false, None).await;
            assert!(!matches!(d, BudgetDecision::Block { .. }), "{mode:?} {d:?}");
            if mode == LimitMode::Off {
                assert_eq!(d, BudgetDecision::Allow);
            }
        }
    }
}

/// Budżet tła domyślnie 0 PLN: płatne zadania tła blokowane, lokalne (0) dozwolone.
pub async fn background_budget_defaults_to_local_only<C: CostMeter>(c: &C) {
    ok(c.set_budget(BudgetConfig::default(), BudgetOrigin::User)
        .await);
    match c.check_budget(1, true, None).await {
        BudgetDecision::Block { notice } => assert_eq!(notice.scope, BudgetScope::Background),
        other => panic!("{other:?}"),
    }
    assert_eq!(c.check_budget(0, true, None).await, BudgetDecision::Allow);
    assert_eq!(c.check_budget(1, false, None).await, BudgetDecision::Allow);
}

/// Limit dostawcy: włączony blokuje, wyłączony tylko ostrzega.
pub async fn provider_limit<C: CostMeter>(c: &C) {
    for enabled in [true, false] {
        let mut providers = BTreeMap::new();
        providers.insert(
            pid("acme"),
            CostLimit {
                enabled,
                monthly_limit_grosze: 100,
            },
        );
        let cfg = BudgetConfig {
            providers,
            ..BudgetConfig::default()
        };
        ok(c.set_budget(cfg, BudgetOrigin::Broker).await);
        let d = c.check_budget(2_000_000, false, Some(&pid("acme"))).await;
        if enabled {
            assert!(
                matches!(
                    d,
                    BudgetDecision::Block { ref notice } if notice.scope == BudgetScope::Provider(pid("acme"))
                ),
                "{d:?}"
            );
        } else {
            assert!(matches!(d, BudgetDecision::Warn { .. }), "{d:?}");
        }
        assert_eq!(
            c.check_budget(2_000_000, false, Some(&pid("other"))).await,
            BudgetDecision::Allow
        );
    }
}

/// Limity są polityką Jądra: Ulepszacz i agentki ich nie zmieniają; zła konfiguracja odrzucona.
pub async fn budget_is_kernel_policy<C: CostMeter>(c: &C) {
    let before = c.budget();
    let loose = BudgetConfig {
        monthly: MonthlyLimit::pln(1_000_000, LimitMode::Off),
        ..BudgetConfig::default()
    };
    for origin in [BudgetOrigin::Improver, BudgetOrigin::Agent("alfa".into())] {
        assert!(matches!(
            c.set_budget(loose.clone(), origin).await,
            Err(CostError::NotPermitted(_))
        ));
    }
    let bad = BudgetConfig {
        fallback_rate_e4: 0,
        ..BudgetConfig::default()
    };
    assert!(matches!(
        c.set_budget(bad, BudgetOrigin::User).await,
        Err(CostError::InvalidConfig(_))
    ));
    assert_eq!(c.budget(), before);
}

/// Uruchamia cały zestaw; `factory()` daje świeżą instancję z kursem kontraktowym.
pub async fn run_all<C, F, Fut>(factory: F)
where
    C: CostMeter,
    F: Fn() -> Fut,
    Fut: Future<Output = C>,
{
    record_converts_and_aggregates(&factory().await).await;
    enforced_limit_blocks(&factory().await).await;
    disabled_limit_never_blocks(&factory().await).await;
    background_budget_defaults_to_local_only(&factory().await).await;
    provider_limit(&factory().await).await;
    budget_is_kernel_policy(&factory().await).await;
}
