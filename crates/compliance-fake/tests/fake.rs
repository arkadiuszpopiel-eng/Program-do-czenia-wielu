//! Testy atrapy: kontrakt współdzielony + sterowanie datą i statusami.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::contract_tests::{self, fixture_catalog, fixture_registry, fresh_day};
use compliance_contract::{Compliance, DecisionReason, RouteId, RouteStatus, SessionTag};
use compliance_fake::FakeCompliance;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|reg, cat, today| async move { FakeCompliance::new(reg, cat, today) })
        .await;
}

#[tokio::test]
async fn controls_today_and_status() {
    let fake = FakeCompliance::new(fixture_registry(), fixture_catalog(), fresh_day());
    let green = RouteId::new("green-cli").unwrap();
    assert!(fake.route_allowed(&green, SessionTag::Standard).allowed);
    fake.advance_days(60);
    assert_eq!(
        fake.route_allowed(&green, SessionTag::Standard).reason,
        DecisionReason::Disabled { stale: true }
    );
    fake.set_today(fresh_day());
    assert!(fake.set_status(&green, RouteStatus::Forbidden));
    assert!(!fake.set_status(&RouteId::new("nope").unwrap(), RouteStatus::Green));
    assert_eq!(
        fake.route_allowed(&green, SessionTag::Private).reason,
        DecisionReason::Forbidden
    );
    let q = fake.queries();
    assert_eq!(q.len(), 3);
    assert_eq!(q[0], (green.clone(), SessionTag::Standard, true));
    assert!(!q[2].2);
}
