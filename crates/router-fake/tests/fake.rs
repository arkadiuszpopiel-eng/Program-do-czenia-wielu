//! Zestaw kontraktowy na atrapie + skryptowanie i zapis wywołań.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use providers_contract::{ChatRequest, Message, ProviderErrorKind, ProviderId};
use router_contract::contract_tests::{self, Harness, policy};
use router_contract::{
    BreakerState, Candidate, Constraints, Outcome, RouteDecision, RouteError, RoutePolicy, Router,
    TaskClass,
};
use router_fake::FakeRouter;

struct H;

impl Harness for H {
    type R = FakeRouter;
    fn router(&self, policy: RoutePolicy) -> FakeRouter {
        FakeRouter::new(policy)
    }
}

#[test]
fn contract_suite_on_fake() {
    contract_tests::run_all(&H);
}

#[test]
fn scripted_routes_calls_and_half_open() {
    let fake = FakeRouter::new(policy());
    let scripted = RouteDecision {
        class: TaskClass::Code,
        chosen: Candidate::new("beta", "mid"),
        fallbacks: vec![],
        rejected: vec![],
        warnings: vec![],
    };
    fake.push_route(TaskClass::Code, Ok(scripted.clone()));
    fake.push_route(
        TaskClass::Code,
        Err(RouteError::NoRoute {
            class: TaskClass::Code,
            rejected: vec![],
        }),
    );
    let req = ChatRequest::new("auto", vec![Message::user_text("x")]);
    assert_eq!(
        fake.route(TaskClass::Code, &Constraints::default(), Some(&req)),
        Ok(scripted)
    );
    assert!(
        fake.route(TaskClass::Code, &Constraints::default(), None)
            .is_err()
    );
    let d = fake
        .route(TaskClass::Code, &Constraints::default(), None)
        .unwrap();
    assert_eq!(d.chosen, Candidate::new("alpha", "big"));
    assert_eq!(fake.calls().len(), 3);
    assert_eq!(fake.calls()[0].model.as_deref(), Some("auto"));
    let alpha = Candidate::new("alpha", "big");
    for _ in 0..3 {
        fake.report(
            &alpha,
            Outcome::Failed {
                kind: ProviderErrorKind::Network,
            },
        );
    }
    fake.report(
        &alpha,
        Outcome::Failed {
            kind: ProviderErrorKind::InvalidRequest,
        },
    );
    let id = ProviderId::new("alpha");
    assert!(matches!(fake.breaker_state(&id), BreakerState::Open { .. }));
    fake.advance_ms(fake.policy().breaker.cooldown_ms);
    assert!(matches!(
        fake.breaker_state(&id),
        BreakerState::HalfOpen { .. }
    ));
    fake.report(
        &alpha,
        Outcome::Ok {
            ttft_ms: Some(100),
            latency_ms: 300,
        },
    );
    assert_eq!(fake.breaker_state(&id), BreakerState::Closed);
    assert_eq!(fake.reports().len(), 5);
}
