//! Zapamiętany poziom globalny (decyzja właściciela 27.10, „Pełny dostęp” wybierany raz):
//! L4 zatwierdzone w oknie Brokera przetrwa restart Brokera bez nowej karty; obniżenie kasuje
//! zapis; poziom czasowy, poziom sesji i prośba agentki nie są zapamiętywane.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;

use core_bus_contract::{AgentId, SessionId};
use safety_broker_contract::contract_tests::approve;
use safety_broker_contract::{
    ApprovalDecision, AutonomyChangeRequest, AutonomyLevel, AutonomyTarget, Broker, ChangeOrigin,
};
use safety_broker_impl::BrokerEngine;
use watchdog_contract::{Clock, ManualClock};

fn broker(path: &Path, clock: &Arc<ManualClock>) -> BrokerEngine {
    common::engine_with(
        safety_broker_contract::contract_tests::test_policy(),
        clock.clone(),
        Arc::new(safety_broker_impl::audit::MemoryAudit::default()),
    )
    .with_standing_level(path.to_path_buf())
}

fn change(
    target: AutonomyTarget,
    level: AutonomyLevel,
    until_ms: Option<u64>,
) -> AutonomyChangeRequest {
    AutonomyChangeRequest {
        target,
        level,
        until_ms,
        origin: ChangeOrigin::UserInterface,
    }
}

fn level(b: &BrokerEngine) -> AutonomyLevel {
    b.autonomy(&SessionId::new("s1"), Some(&AgentId::new("delta")))
}

#[tokio::test]
async fn approved_global_l4_survives_restart_until_owner_lowers_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&path, &clock);
    let id = b
        .request_autonomy_change(change(AutonomyTarget::Global, AutonomyLevel::L4, None))
        .await
        .unwrap()
        .expect("podniesienie otwiera kartę");
    assert_eq!(level(&b), AutonomyLevel::L3, "przed zgodą bez zmian");
    approve(&b, &clock, id, ApprovalDecision::Allow).await;
    assert_eq!(level(&b), AutonomyLevel::L4);
    drop(b);

    // Restart Brokera: L4 obowiązuje od razu, bez karty; ta sama prośba niczego nie otwiera.
    let b = broker(&path, &clock);
    assert_eq!(level(&b), AutonomyLevel::L4);
    let again = b
        .request_autonomy_change(change(AutonomyTarget::Global, AutonomyLevel::L4, None))
        .await;
    assert_eq!(again, Ok(None));

    // Właściciel obniża — zapis znika, kolejny restart daje poziom domyślny.
    let lower = change(AutonomyTarget::Global, AutonomyLevel::L3, None);
    assert_eq!(b.request_autonomy_change(lower).await, Ok(None));
    assert!(!path.exists());
    drop(b);
    assert_eq!(level(&broker(&path, &clock)), AutonomyLevel::L3);
}

#[tokio::test]
async fn timed_session_and_denied_raises_are_not_remembered() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&path, &clock);
    let timed = change(
        AutonomyTarget::Global,
        AutonomyLevel::L4,
        Some(clock.now_ms() + 60_000),
    );
    let id = b.request_autonomy_change(timed).await.unwrap().unwrap();
    approve(&b, &clock, id, ApprovalDecision::Allow).await;
    let session = AutonomyTarget::Session {
        session: SessionId::new("s1"),
    };
    let id = b
        .request_autonomy_change(change(session, AutonomyLevel::L4, None))
        .await
        .unwrap()
        .unwrap();
    approve(&b, &clock, id, ApprovalDecision::Allow).await;
    let id = b
        .request_autonomy_change(change(AutonomyTarget::Global, AutonomyLevel::L4, None))
        .await
        .unwrap()
        .unwrap();
    approve(&b, &clock, id, ApprovalDecision::Deny).await;
    assert!(!path.exists(), "czasowe, sesji i odrzucone — bez zapisu");
    // Agentka nie podniesie poziomu ani prośbą, ani przez zapis.
    let mut agent = change(AutonomyTarget::Global, AutonomyLevel::L4, None);
    agent.origin = ChangeOrigin::Agent("delta".into());
    assert!(b.request_autonomy_change(agent).await.is_err());
    assert!(!path.exists());
}
