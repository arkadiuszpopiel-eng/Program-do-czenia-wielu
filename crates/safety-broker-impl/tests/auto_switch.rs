//! „Zezwól i przełącz na Automatycznie” (plan 50 §4.2, Z.3): okno proponuje tę opcję tylko przy
//! pojedynczej akcji, gdy tryb globalny jest niższy niż L3 i podmiot nie ma własnego poziomu.
//! Wybór zezwala raz i podnosi tryb globalny do L3 (rdzeń przyjmuje go jako zapamiętany); poza tymi
//! warunkami Sędzia odrzuca decyzję, a prośba czeka dalej.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;

use safety_broker_contract::contract_tests::{
    approve, challenge, delta, exact, needs_approval, proof, request, set_level, test_policy,
};
use safety_broker_contract::{
    ApprovalChannel, ApprovalDecision, AutonomyLevel, AutonomyTarget, Broker, BrokerError,
    Capability, CommandOrigin,
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

fn write() -> safety_broker_contract::ActionRequest {
    request(
        &delta(),
        Capability::FsWrite(exact(r"C:\Users\Ala\Documents\raport.txt")),
        CommandOrigin::Agent,
    )
}

#[tokio::test]
async fn switch_allows_once_and_raises_the_global_mode_for_good() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&path, &clock);
    set_level(&b, &clock, AutonomyTarget::Global, AutonomyLevel::L1).await;
    assert_eq!(b.metrics().standing_global, Some(AutonomyLevel::L1));
    let ticket = needs_approval(b.decide(write()).await);
    assert!(challenge(&b, ticket.id).request.auto_switch);
    approve(&b, &clock, ticket.id, ApprovalDecision::AllowAndAutomatic).await;
    assert_eq!(b.metrics().standing_global, Some(AutonomyLevel::L3));
    let holder = delta();
    assert_eq!(
        b.autonomy(&holder.session, holder.agent.as_ref()),
        AutonomyLevel::L3
    );
    // Po restarcie Sędziego L3 jest poziomem domyślnym (zapis na dysku tylko powyżej L3).
    drop(b);
    let b = broker(&path, &clock);
    assert_eq!(
        b.autonomy(&holder.session, holder.agent.as_ref()),
        AutonomyLevel::L3
    );
}

#[tokio::test]
async fn no_switch_for_own_session_level_or_when_already_automatic() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&dir.path().join("s.json"), &clock);
    // Rozmowa z własnym poziomem (np. Plan) — zmiana trybu globalnego by jej nie objęła.
    let session = AutonomyTarget::Session {
        session: delta().session,
    };
    set_level(&b, &clock, session, AutonomyLevel::L1).await;
    let ticket = needs_approval(b.decide(write()).await);
    let ch = challenge(&b, ticket.id);
    assert!(!ch.request.auto_switch);
    let refused = b
        .resolve(
            ticket.id,
            ApprovalDecision::AllowAndAutomatic,
            proof(&ch, &clock, false),
        )
        .await;
    assert!(matches!(refused, Err(BrokerError::InvalidRequest(_))));
    assert!(b.pending().iter().any(|c| c.request.id == ticket.id));
    assert_eq!(b.metrics().standing_global, None);

    // Tryb „Automatycznie”: pytanie o rzecz nieodwracalną — bez opcji przełączenia.
    let dir = tempfile::tempdir().unwrap();
    let b = broker(&dir.path().join("s.json"), &clock);
    let mut risky = request(
        &delta(),
        Capability::FsWrite(exact(r"D:\dane\raport.txt")),
        CommandOrigin::Agent,
    );
    risky.facts.reversible = risk_classifier_contract::Reversibility::No;
    let ticket = needs_approval(b.decide(risky).await);
    assert!(!challenge(&b, ticket.id).request.auto_switch);
}
