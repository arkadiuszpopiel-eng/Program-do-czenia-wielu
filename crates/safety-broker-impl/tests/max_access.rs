//! „Max dostęp” (decyzja właściciela 3.11, plan 50 §8.1): jeden przełącznik z zasięgiem modele
//! lokalne / w chmurze / wszystkie. Włączenie tylko przez okno Sędziego (agentka nie może),
//! wtedy przebiegi na modelach z zasięgu nie pytają (twarde blokady Jądra zostają), stan przetrwa
//! restart, wyłączenie działa od razu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;

use safety_broker_contract::contract_tests::{
    allowed, approve, delta, exact, needs_approval, request, test_policy,
};
use safety_broker_contract::{
    ActionRequest, ApprovalChannel, ApprovalDecision, AutonomyChangeRequest, AutonomyLevel,
    AutonomyTarget, Broker, BrokerError, Capability, ChangeOrigin, CommandOrigin, Decision,
    DenyReason, KernelRule, ModelLocation, ModelScope,
};
use safety_broker_impl::BrokerEngine;
use watchdog_contract::ManualClock;

fn broker(path: &Path, clock: &Arc<ManualClock>) -> BrokerEngine {
    common::engine_with(
        test_policy(),
        clock.clone(),
        Arc::new(safety_broker_impl::audit::MemoryAudit::default()),
    )
    .with_standing_level(path.to_path_buf())
}

fn max(scope: ModelScope, level: AutonomyLevel, origin: ChangeOrigin) -> AutonomyChangeRequest {
    AutonomyChangeRequest {
        target: AutonomyTarget::Models { scope },
        level,
        until_ms: None,
        origin,
    }
}

/// Zapis nieodwracalny poza profilem — na „Automatycznie” (L3) pyta, na L4 nie.
fn risky(location: Option<ModelLocation>) -> ActionRequest {
    let mut r = request(
        &delta(),
        Capability::FsWrite(exact(r"D:\dane\raport.txt")),
        CommandOrigin::Agent,
    );
    r.facts.reversible = risk_classifier_contract::Reversibility::No;
    r.facts.model_location = location;
    r
}

async fn turn_on(b: &BrokerEngine, clock: &ManualClock, scope: ModelScope) {
    let before = b.metrics().max_access;
    let id = b
        .request_autonomy_change(max(scope, AutonomyLevel::L4, ChangeOrigin::UserInterface))
        .await
        .unwrap()
        .expect("włączenie otwiera kartę Sędziego");
    assert_eq!(b.metrics().max_access, before, "przed zgodą bez zmian");
    approve(b, clock, id, ApprovalDecision::Allow).await;
    assert_eq!(b.metrics().max_access, Some(scope));
}

#[tokio::test]
async fn local_max_access_covers_only_local_runs_and_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&path, &clock);
    needs_approval(b.decide(risky(Some(ModelLocation::Local))).await);
    turn_on(&b, &clock, ModelScope::Local).await;
    allowed(b.decide(risky(Some(ModelLocation::Local))).await);
    needs_approval(b.decide(risky(Some(ModelLocation::Cloud))).await);
    needs_approval(b.decide(risky(None)).await);
    // Twarde blokady Jądra zostają (np. zapis w katalogu Windows).
    let mut kernel = request(
        &delta(),
        Capability::FsWrite(exact(r"C:\Windows\System32\drivers\etc\hosts")),
        CommandOrigin::Agent,
    );
    kernel.facts.model_location = Some(ModelLocation::Local);
    assert!(matches!(
        b.decide(kernel).await,
        Ok(Decision::Deny(DenyReason::KernelBlock(_)))
    ));
    drop(b);

    // Restart: włączony bez nowej karty.
    let b = broker(&path, &clock);
    assert_eq!(b.metrics().max_access, Some(ModelScope::Local));
    allowed(b.decide(risky(Some(ModelLocation::Local))).await);

    // Wyłączenie działa od razu i kasuje zapis.
    let off = max(
        ModelScope::Local,
        AutonomyLevel::L3,
        ChangeOrigin::UserInterface,
    );
    assert_eq!(b.request_autonomy_change(off).await, Ok(None));
    assert_eq!(b.metrics().max_access, None);
    needs_approval(b.decide(risky(Some(ModelLocation::Local))).await);
    assert!(!path.exists());
}

#[tokio::test]
async fn one_switch_cloud_and_all_scopes() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&dir.path().join("s.json"), &clock);
    turn_on(&b, &clock, ModelScope::Cloud).await;
    allowed(b.decide(risky(Some(ModelLocation::Cloud))).await);
    // Działanie bez modelu (wtyczka, zrzut ekranu) — nie „chmura”, tylko „wszystkie” (plan 50 §8.2).
    needs_approval(b.decide(risky(None)).await);
    needs_approval(b.decide(risky(Some(ModelLocation::Local))).await);
    // Zmiana zasięgu na „wszystkie” — nowa karta, potem jeden wpis.
    turn_on(&b, &clock, ModelScope::All).await;
    allowed(b.decide(risky(Some(ModelLocation::Local))).await);
    allowed(b.decide(risky(Some(ModelLocation::Cloud))).await);
    allowed(b.decide(risky(None)).await);
}

#[tokio::test]
async fn agent_cannot_turn_on_max_access() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&dir.path().join("s.json"), &clock);
    let raise = max(
        ModelScope::All,
        AutonomyLevel::L4,
        ChangeOrigin::Agent("delta".into()),
    );
    assert_eq!(
        b.request_autonomy_change(raise).await,
        Err(BrokerError::KernelBlock(KernelRule::SelfEscalation))
    );
    assert!(b.pending().is_empty());
    assert_eq!(b.metrics().max_access, None);
}

#[tokio::test]
async fn owner_denial_keeps_max_access_off() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&dir.path().join("s.json"), &clock);
    let id = b
        .request_autonomy_change(max(
            ModelScope::Local,
            AutonomyLevel::L4,
            ChangeOrigin::UserInterface,
        ))
        .await
        .unwrap()
        .expect("karta");
    approve(&b, &clock, id, ApprovalDecision::Deny).await;
    assert_eq!(b.metrics().max_access, None);
    needs_approval(b.decide(risky(Some(ModelLocation::Local))).await);
}
