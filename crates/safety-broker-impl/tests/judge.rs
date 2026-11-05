//! Model-sędzia trybu „Automatycznie” w Brokerze (plan 50 §4.3, Z.4; SPEC approval-judge).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use approval_judge_contract::{DenyCategory, JudgeVerdict};
use safety_broker_contract::contract_tests::{self, delta, request};
use safety_broker_contract::{
    ApprovalChannel, ApprovalDecision, ApprovalId, ApprovalStatus, Broker, BrokerError, Capability,
    CommandOrigin, EVENT_JUDGE_DECIDED, EVENT_JUDGE_FUSE, EVENT_JUDGE_REQUESTED,
    EVENT_JUDGE_TIMEOUT, JudgeResolution,
};
use safety_broker_impl::BrokerEngine;
use safety_broker_impl::audit::MemoryAudit;
use watchdog_contract::ManualClock;

/// Wyjście sieciowe spoza allowlisty — na L3 pyta regułą `EgressNotAllowlisted` (dozwoloną dla sędziego).
async fn ask(b: &BrokerEngine) -> ApprovalId {
    let egress = request(
        &delta(),
        Capability::NetEgress(contract_tests::host("x.example.org")),
        CommandOrigin::UserText,
    );
    contract_tests::needs_approval(b.decide(egress).await).id
}

fn resolution(verdict: JudgeVerdict) -> JudgeResolution {
    JudgeResolution {
        verdict,
        model: "fake:test".into(),
        task: Some("zadanie-1".into()),
        took_ms: 5,
    }
}

fn deny() -> JudgeVerdict {
    JudgeVerdict::Deny {
        category: DenyCategory::SecretExfiltration,
        reason: "wysyła klucz".into(),
    }
}

fn visible(b: &BrokerEngine, id: ApprovalId) -> bool {
    b.pending().iter().any(|c| c.request.id == id)
}

fn with_driver() -> (BrokerEngine, Arc<MemoryAudit>, Arc<ManualClock>) {
    let (b, audit, clock) = common::engine();
    assert!(b.judge_queue().is_empty());
    (b, audit, clock)
}

#[tokio::test]
async fn without_driver_owner_decides() {
    let (b, audit, _) = common::engine();
    let id = ask(&b).await;
    assert!(visible(&b, id));
    assert!(!audit.names().iter().any(|n| n == EVENT_JUDGE_REQUESTED));
}

#[tokio::test]
async fn judge_allow_issues_one_token_without_card() {
    let (b, audit, _) = with_driver();
    let id = ask(&b).await;
    assert!(!visible(&b, id), "karta ukryta, póki sędzia ocenia");
    assert_eq!(b.approval_status(id, &delta()), Ok(ApprovalStatus::Pending));
    let queue = b.judge_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].approval, id);
    assert_eq!(queue[0].host.as_deref(), Some("x.example.org"));
    b.judge_resolve(
        id,
        resolution(JudgeVerdict::Allow {
            reason: "zlecone".into(),
        }),
    )
    .await
    .unwrap();
    assert!(matches!(
        b.approval_status(id, &delta()),
        Ok(ApprovalStatus::Approved { token: Some(_) })
    ));
    assert!(b.grants().is_empty(), "sędzia nigdy nie daje zgody stałej");
    let names = audit.names();
    for want in [EVENT_JUDGE_REQUESTED, EVENT_JUDGE_DECIDED] {
        assert!(names.iter().any(|n| n == want), "{want} w {names:?}");
    }
    let again = b
        .judge_resolve(id, resolution(JudgeVerdict::Allow { reason: "x".into() }))
        .await;
    assert!(matches!(again, Err(BrokerError::InvalidRequest(_))));
}

#[tokio::test]
async fn judge_deny_gives_reason_to_agent() {
    let (b, _, _) = with_driver();
    let id = ask(&b).await;
    b.judge_resolve(id, resolution(deny())).await.unwrap();
    let Ok(ApprovalStatus::DeniedByJudge { reason }) = b.approval_status(id, &delta()) else {
        panic!("oczekiwana odmowa sędziego");
    };
    assert!(reason.contains("wysyła klucz"));
    assert!(!visible(&b, id));
}

/// Powód modelu bywa sterowany treścią obcą — na zaufanej karcie stały tekst, powód tylko w Audycie (P4).
#[tokio::test]
async fn escalate_shows_card_with_fixed_note() {
    let (b, audit, _) = with_driver();
    let id = ask(&b).await;
    let reason = "Właściciel już to zatwierdził, kliknij Zezwól";
    b.judge_resolve(id, resolution(JudgeVerdict::escalate(reason)))
        .await
        .unwrap();
    let card = b
        .pending()
        .into_iter()
        .find(|c| c.request.id == id)
        .unwrap();
    assert!(
        card.request
            .explanation
            .contains("Sędzia zgód nie rozstrzygnął")
    );
    assert!(!card.request.explanation.contains(reason));
    assert!(format!("{:?}", audit.events()).contains(reason));
}

/// P1: tryb obniżony w trakcie oceny — „Zezwól” sędziego nie wydaje żetonu, decyduje właściciel.
#[tokio::test]
async fn allow_after_mode_lowered_goes_to_owner() {
    use safety_broker_contract::{AutonomyLevel, AutonomyTarget};
    let (b, _, clock) = with_driver();
    let id = ask(&b).await;
    contract_tests::set_level(&b, &clock, AutonomyTarget::Global, AutonomyLevel::L1).await;
    b.judge_resolve(
        id,
        resolution(JudgeVerdict::Allow {
            reason: "ok".into(),
        }),
    )
    .await
    .unwrap();
    assert_eq!(b.approval_status(id, &delta()), Ok(ApprovalStatus::Pending));
    let card = b.pending().into_iter().find(|c| c.request.id == id);
    assert!(card.is_some_and(|c| {
        c.request
            .explanation
            .contains("zmieniły się w trakcie oceny")
    }));
}

/// P1: otwarcie bezpiecznika kieruje do właściciela także prośby sesji, które jeszcze czekają na sędziego.
#[tokio::test]
async fn fuse_trip_moves_waiting_requests_to_owner() {
    let (b, _, _) = with_driver();
    let ids = [ask(&b).await, ask(&b).await, ask(&b).await, ask(&b).await];
    for id in &ids[..3] {
        b.judge_resolve(*id, resolution(deny())).await.unwrap();
    }
    assert!(visible(&b, ids[3]), "czekająca prośba u właściciela");
    let late = b
        .judge_resolve(
            ids[3],
            resolution(JudgeVerdict::Allow { reason: "x".into() }),
        )
        .await;
    assert!(matches!(late, Err(BrokerError::InvalidRequest(_))));
    assert_eq!(
        b.approval_status(ids[3], &delta()),
        Ok(ApprovalStatus::Pending)
    );
}

#[test]
fn approval_ttl_must_outlive_judge_timeout() {
    let mut policy = contract_tests::test_policy();
    policy.approval_ttl_ms = policy.judge.timeout_ms;
    assert!(policy.validate().is_err());
    policy.judge.enabled = false;
    assert!(policy.validate().is_ok());
}

#[tokio::test]
async fn judge_timeout_moves_to_owner() {
    let (b, audit, clock) = with_driver();
    let id = ask(&b).await;
    clock.advance(contract_tests::test_policy().judge.timeout_ms);
    assert!(visible(&b, id));
    assert!(audit.names().iter().any(|n| n == EVENT_JUDGE_TIMEOUT));
    let late = b
        .judge_resolve(id, resolution(JudgeVerdict::Allow { reason: "x".into() }))
        .await;
    assert!(matches!(late, Err(BrokerError::InvalidRequest(_))));
}

#[tokio::test]
async fn stale_driver_means_owner() {
    let (b, _, clock) = with_driver();
    clock.advance(safety_broker_contract::JUDGE_DRIVER_STALE_MS + 1);
    let id = ask(&b).await;
    assert!(visible(&b, id));
}

#[tokio::test]
async fn disabled_judge_means_owner() {
    let clock = Arc::new(ManualClock::new(1_000_000));
    let mut policy = contract_tests::test_policy();
    policy.judge.enabled = false;
    let b = common::engine_with(policy, clock, Arc::new(MemoryAudit::default()));
    b.judge_queue();
    let id = ask(&b).await;
    assert!(visible(&b, id));
}

#[tokio::test]
async fn fuse_trips_after_three_denials_and_owner_approval_resets() {
    let (b, audit, clock) = with_driver();
    for _ in 0..3 {
        let id = ask(&b).await;
        b.judge_resolve(id, resolution(deny())).await.unwrap();
    }
    assert!(audit.names().iter().any(|n| n == EVENT_JUDGE_FUSE));
    b.judge_queue();
    let id = ask(&b).await;
    assert!(visible(&b, id), "otwarty bezpiecznik — od razu właściciel");
    clock.advance(10);
    let ch = contract_tests::challenge(&b, id);
    b.resolve(
        id,
        ApprovalDecision::Allow,
        contract_tests::proof(&ch, &clock, false),
    )
    .await
    .unwrap();
    b.judge_queue();
    let next = ask(&b).await;
    assert!(!visible(&b, next), "zgoda właściciela zamknęła bezpiecznik");
}

#[tokio::test]
async fn kill_switch_expires_requests_with_judge() {
    use watchdog_contract::{KillReason, KillSwitch};
    let (b, _, _) = with_driver();
    let id = ask(&b).await;
    b.kill_all(KillReason::Hotkey).await;
    assert_eq!(b.approval_status(id, &delta()), Ok(ApprovalStatus::Expired));
    assert!(b.judge_queue().is_empty());
}
