//! Zgody „zawsze w tym zakresie” z czasem wybranym przez właściciela (31.10: 1 h, 3 h, doba albo
//! do odwołania): termin przycinany do polityki, „do odwołania” bez terminu i zapamiętane po
//! restarcie Brokera, lista i odwołanie z panelu uprawnień, STOP kasuje wszystkie (i zapis).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;

use risk_classifier_contract::CommandOrigin;
use safety_broker_contract::contract_tests::{
    allowed, approve, delta, host, needs_approval, request,
};
use safety_broker_contract::{ApprovalDecision, Broker, Capability, UNTIL_REVOKED};
use safety_broker_impl::BrokerEngine;
use watchdog_contract::{Clock, KillReason, KillSwitch, ManualClock};

fn broker(path: &Path, clock: &Arc<ManualClock>) -> BrokerEngine {
    common::engine_with(
        safety_broker_contract::contract_tests::test_policy(),
        clock.clone(),
        Arc::new(safety_broker_impl::audit::MemoryAudit::default()),
    )
    .with_standing_level(path.to_path_buf())
}

fn ask(h: &str) -> safety_broker_contract::ActionRequest {
    request(
        &delta(),
        Capability::NetEgress(host(h)),
        CommandOrigin::UserText,
    )
}

/// Zgoda na „*.example.org” do `until_ms`.
async fn grant(b: &BrokerEngine, clock: &ManualClock, until_ms: u64) {
    let ticket = needs_approval(b.decide(ask("a.example.org")).await);
    let scope = Capability::NetEgress(host("*.example.org"));
    approve(
        b,
        clock,
        ticket.id,
        ApprovalDecision::AllowInScope { scope, until_ms },
    )
    .await;
}

#[tokio::test]
async fn until_revoked_survives_restart_and_ends_on_revoke() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&path, &clock);
    grant(&b, &clock, UNTIL_REVOKED).await;
    let list = b.grants();
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].until_ms, UNTIL_REVOKED,
        "bez terminu — nie przycinane"
    );
    assert_eq!(list[0].scope, Capability::NetEgress(host("*.example.org")));
    drop(b);

    // Restart Brokera (i tydzień później): zgoda nadal działa bez karty.
    clock.advance(7 * 24 * 3_600_000);
    let b = broker(&path, &clock);
    allowed(b.decide(ask("b.example.org")).await);
    let id = b.grants()[0].id;

    // Odwołanie w panelu: od razu znowu pytanie, zapis znika, drugie odwołanie — `false`.
    assert_eq!(b.revoke_grant(id).await, Ok(true));
    needs_approval(b.decide(ask("b.example.org")).await);
    assert!(b.grants().is_empty());
    assert!(!dir.path().join("grants-standing.json").exists());
    assert_eq!(b.revoke_grant(id).await, Ok(false));
    drop(b);
    assert!(
        broker(&path, &clock).grants().is_empty(),
        "po restarcie też"
    );
}

#[tokio::test]
async fn timed_grants_are_capped_listed_and_not_remembered() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&path, &clock);
    let now = clock.now_ms();
    grant(&b, &clock, now + 3 * 3_600_000).await;
    assert_eq!(b.grants()[0].until_ms, now + 3 * 3_600_000);
    assert!(
        !dir.path().join("grants-standing.json").exists(),
        "z terminem — tylko w pamięci"
    );
    clock.advance(3 * 3_600_000);
    assert!(b.grants().is_empty(), "wygasła zgoda znika z listy");
    needs_approval(b.decide(ask("b.example.org")).await);
}

#[tokio::test]
async fn stop_clears_every_grant_and_the_saved_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&path, &clock);
    grant(&b, &clock, UNTIL_REVOKED).await;
    assert!(dir.path().join("grants-standing.json").exists());
    b.kill_all(KillReason::Hotkey).await;
    assert!(b.grants().is_empty());
    assert!(!dir.path().join("grants-standing.json").exists());
    drop(b);
    let b = broker(&path, &clock);
    assert!(
        b.grants().is_empty(),
        "STOP przetrwa restart — zgody nie wracają"
    );
}
