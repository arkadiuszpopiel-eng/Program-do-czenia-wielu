//! Przegląd 2026-10-22: Router blokuje według kosztu minimalnego (jak budżet rozmowy poza
//! Routerem), a pełne `max_tokens` modelu daje tylko ostrzeżenie — wcześniej przy limicie 100 zł
//! rozmowa stawała przy ok. 90 zł, bo każda wiadomość „kosztowała” pełne 128 tys. tokenów wyjścia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cost_meter_contract::{BudgetDecision, BudgetNotice, BudgetScope};
use providers_contract::{ModelProvider, ProviderId};
use providers_fake::FAKE_MODEL;
use router_contract::{
    BudgetGate, Constraints, RejectReason, RouteKind, RoutePolicy, RouteWarning, Router, TaskClass,
};
use router_impl::RouterCore;
use support::{cand, fake, req};

/// Bramka z limitem w mikro-USD: powyżej — blokada.
struct Limit(AtomicU64);

impl BudgetGate for Limit {
    fn check(&self, _provider: &ProviderId, micro_usd: u64, _background: bool) -> BudgetDecision {
        let limit = self.0.load(Ordering::SeqCst);
        if micro_usd <= limit {
            return BudgetDecision::Allow;
        }
        BudgetDecision::Block {
            notice: BudgetNotice {
                scope: BudgetScope::Monthly,
                spent_micro_pln: 0,
                estimate_micro_pln: micro_usd,
                limit_micro_pln: limit,
                pct_after: 101,
            },
        }
    }
}

#[test]
fn blocks_by_minimum_and_warns_by_maximum() {
    let r = req(&"koszt ".repeat(2_000));
    let mut priced = r.clone();
    priced.model = FAKE_MODEL.into();
    let estimate = fake("alpha").estimate_cost(&priced).unwrap();
    let (min, max) = (estimate.min.micro_usd_ceil(), estimate.max.micro_usd_ceil());
    assert!(min < max, "{min} < {max}");
    let gate = Arc::new(Limit(AtomicU64::new(min)));
    let core = RouterCore::new(None, Some(gate.clone()));
    core.register(Arc::new(fake("alpha")), RouteKind::Api);
    core.set_policy(Some(RoutePolicy::defaults(None, &[cand("alpha")])));
    // Limit między minimum a maksimum: rozmowa idzie, z ostrzeżeniem.
    let d = core
        .route(TaskClass::Conversation, &Constraints::default(), Some(&r))
        .unwrap();
    assert_eq!(d.chosen.provider.as_str(), "alpha");
    assert!(
        d.warnings
            .iter()
            .any(|(_, w)| matches!(w, RouteWarning::Budget { .. })),
        "{:?}",
        d.warnings
    );
    // Limit poniżej minimum: odmowa z czytelną nazwą limitu.
    gate.0.store(min.saturating_sub(1), Ordering::SeqCst);
    let blocked = core.route(TaskClass::Conversation, &Constraints::default(), Some(&r));
    let text = format!("{blocked:?}");
    assert!(text.contains("Budget"), "{text}");
    let reason = RejectReason::Budget {
        notice: BudgetNotice {
            scope: BudgetScope::Monthly,
            spent_micro_pln: 0,
            estimate_micro_pln: 1,
            limit_micro_pln: 1,
            pct_after: 101,
        },
    };
    assert_eq!(
        reason.to_string(),
        "limit kosztów miesięczny — 101 % po zadaniu"
    );
}
