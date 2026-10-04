//! Kontrakt współdzielony na implementacji + manifest + Audyt każdej decyzji + fail-closed.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use core_registry_contract::{Isolation, Lifecycle, ModuleManifest};
use safety_broker_contract::contract_tests::{self, delta, request, tree};
use safety_broker_contract::{
    Broker, Capability, CommandOrigin, Decision, DenyReason, EVENT_APPROVAL_REQUESTED,
    EVENT_KERNEL_BLOCK, EVENT_TOKEN_ISSUED,
};
use safety_broker_impl::audit::MemoryAudit;
use safety_broker_impl::{BrokerConfig, BrokerEngine, KeyMode, MODULE_TOML};

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|policy, clock| {
        common::engine_with(policy, clock, Arc::new(MemoryAudit::default()))
    })
    .await;
}

#[test]
fn manifest_is_valid() {
    let m = ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "safety-broker");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.isolation, Isolation::Process);
    assert_eq!(m.lifecycle, Lifecycle::Always);
}

#[test]
fn invalid_policy_prevents_start() {
    let mut policy = contract_tests::test_policy();
    policy.token_ttl_default_ms = 0;
    let config = BrokerConfig {
        policy,
        env: common::env(),
        key_mode: KeyMode::Random,
    };
    let clock = Arc::new(watchdog_contract::ManualClock::new(0));
    let r = BrokerEngine::new(
        config,
        clock,
        Arc::new(MemoryAudit::default()),
        Arc::new(common::RecordingProcesses::default()),
    );
    assert!(r.is_err());
}

#[tokio::test]
async fn every_decision_is_audited() {
    let (b, audit, _) = common::engine();
    let read = request(
        &delta(),
        Capability::FsRead(tree(r"C:\Users\ala\Docs")),
        CommandOrigin::UserText,
    );
    assert!(matches!(b.decide(read).await, Ok(Decision::Allow(_))));
    let egress = request(
        &delta(),
        Capability::NetEgress(contract_tests::host("x.example.org")),
        CommandOrigin::UserText,
    );
    assert!(matches!(
        b.decide(egress).await,
        Ok(Decision::NeedsApproval(_))
    ));
    let creds = request(
        &delta(),
        Capability::FsRead(tree(r"%USERPROFILE%\.codex")),
        CommandOrigin::UserText,
    );
    assert!(matches!(b.decide(creds).await, Ok(Decision::Deny(_))));
    let names = audit.names();
    for want in [
        EVENT_TOKEN_ISSUED,
        EVENT_APPROVAL_REQUESTED,
        EVENT_KERNEL_BLOCK,
    ] {
        assert!(names.iter().any(|n| n == want), "{want} w {names:?}");
    }
    let events = audit.events();
    assert!(
        events
            .iter()
            .all(|e| e.level == core_bus_contract::Level::Audit)
    );
    assert!(events.iter().skip(1).all(|e| e.prev_hash.is_some()));
}

#[tokio::test]
async fn no_token_without_audit() {
    let (b, audit, _) = common::engine();
    audit.set_failing(true);
    let read = request(
        &delta(),
        Capability::FsRead(tree(r"C:\Users\ala\Docs")),
        CommandOrigin::UserText,
    );
    assert_eq!(
        b.decide(read.clone()).await,
        Ok(Decision::Deny(DenyReason::AuditUnavailable))
    );
    let egress = request(
        &delta(),
        Capability::NetEgress(contract_tests::host("x.example.org")),
        CommandOrigin::UserText,
    );
    assert_eq!(
        b.decide(egress).await,
        Ok(Decision::Deny(DenyReason::AuditUnavailable))
    );
    assert_eq!(b.metrics().active_tokens, 0);
    audit.set_failing(false);
    assert!(matches!(b.decide(read).await, Ok(Decision::Allow(_))));
}

#[tokio::test]
async fn key_rotation_keeps_live_tokens_until_grace() {
    let (b, _, clock) = common::engine();
    let read = request(
        &delta(),
        Capability::FsRead(tree(r"C:\Users\ala\Docs")),
        CommandOrigin::UserText,
    );
    let Ok(Decision::Allow(t)) = b.decide(read.clone()).await else {
        panic!("brak tokenu")
    };
    b.rotate_keys().unwrap();
    let file = Capability::FsRead(contract_tests::exact(r"C:\Users\ala\Docs\a.txt"));
    assert_eq!(b.verify(&t, &file, &delta()), Ok(()));
    let Ok(Decision::Allow(t2)) = b.decide(read).await else {
        panic!("brak tokenu")
    };
    assert_eq!(t2.key_epoch, t.key_epoch + 1);
    clock.advance(contract_tests::test_policy().token_ttl_default_ms);
    assert!(b.verify(&t, &file, &delta()).is_err());
}

/// Przegląd Q-9: odmowa właściciela obowiązuje także przy awarii Audytu (błąd zgłoszony
/// Broker-UI); zgoda bez zapisu w Audycie nie przechodzi — prośba czeka dalej, bez tokenu.
#[tokio::test]
async fn owner_denial_applies_even_when_audit_fails() {
    use safety_broker_contract::{ApprovalChannel, ApprovalDecision, ApprovalStatus, BrokerError};
    let (b, audit, clock) = common::engine();
    let egress = || {
        request(
            &delta(),
            Capability::NetEgress(contract_tests::host("x.example.org")),
            CommandOrigin::UserText,
        )
    };
    let denied = contract_tests::needs_approval(b.decide(egress()).await).id;
    let allowed = contract_tests::needs_approval(b.decide(egress()).await).id;
    clock.advance(10);
    audit.set_failing(true);
    let ch = contract_tests::challenge(&b, denied);
    let r = b
        .resolve(
            denied,
            ApprovalDecision::Deny,
            contract_tests::proof(&ch, &clock, false),
        )
        .await;
    assert!(matches!(r, Err(BrokerError::AuditUnavailable(_))), "{r:?}");
    assert_eq!(
        b.approval_status(denied, &delta()),
        Ok(ApprovalStatus::Denied)
    );
    let ch = contract_tests::challenge(&b, allowed);
    let r = b
        .resolve(
            allowed,
            ApprovalDecision::Allow,
            contract_tests::proof(&ch, &clock, false),
        )
        .await;
    assert!(matches!(r, Err(BrokerError::AuditUnavailable(_))), "{r:?}");
    assert_eq!(
        b.approval_status(allowed, &delta()),
        Ok(ApprovalStatus::Pending)
    );
    assert_eq!(b.metrics().active_tokens, 0);
    audit.set_failing(false);
    let ch = contract_tests::challenge(&b, allowed);
    b.resolve(
        allowed,
        ApprovalDecision::Allow,
        contract_tests::proof(&ch, &clock, false),
    )
    .await
    .unwrap();
    assert!(matches!(
        b.approval_status(allowed, &delta()),
        Ok(ApprovalStatus::Approved { token: Some(_) })
    ));
}
