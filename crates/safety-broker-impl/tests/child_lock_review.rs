//! Profil Dziecko — scenariusze z przeglądu 4.11 (plan 50 §8.2): karty i tokeny sprzed zamknięcia
//! kłódki nie działają po nim, poszerzenie bez wpisu w Audycie nie obowiązuje, kłódka otwarta
//! „do zamknięcia Alfy” zamyka się po rozłączeniu aplikacji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;

use safety_broker_contract::contract_tests::{
    allowed, approve, challenge, delta, exact, needs_approval, proof, request, test_policy,
};
use safety_broker_contract::{
    ApprovalChannel, ApprovalDecision, AutonomyChangeRequest, AutonomyLevel, AutonomyTarget,
    Broker, Capability, ChangeOrigin, ChildLockChange, CommandOrigin, Pin,
};
use safety_broker_impl::audit::MemoryAudit;
use safety_broker_impl::{BrokerEngine, CHILD_LOCK_FILE, CoreSession, PinParams};
use watchdog_contract::ManualClock;

fn broker(dir: &Path, clock: &Arc<ManualClock>, audit: &Arc<MemoryAudit>) -> BrokerEngine {
    common::engine_with(test_policy(), clock.clone(), audit.clone())
        .with_standing_level(dir.join("autonomy-standing.json"))
        .with_child_lock(dir.join(CHILD_LOCK_FILE), PinParams::FAST_FOR_TESTS, true)
}

fn pin() -> Pin {
    Pin::new("2468").unwrap()
}

fn level(b: &BrokerEngine) -> AutonomyLevel {
    let h = delta();
    b.autonomy(&h.session, h.agent.as_ref())
}

async fn set_pin(b: &BrokerEngine) {
    let change = ChildLockChange::SetPin {
        old: None,
        new: pin(),
    };
    b.change_child_lock(change).await.unwrap();
}

async fn enable(b: &BrokerEngine) {
    let on = ChildLockChange::Enable {
        pin: pin(),
        on: true,
    };
    b.change_child_lock(on).await.unwrap();
}

/// Nieodwracalny zapis poza profilem — pytanie także na L3.
fn risky() -> safety_broker_contract::ActionRequest {
    let mut r = request(
        &delta(),
        Capability::FsWrite(exact(r"D:\dane\raport.txt")),
        CommandOrigin::Agent,
    );
    r.facts.reversible = risk_classifier_contract::Reversibility::No;
    r
}

fn in_profile() -> safety_broker_contract::ActionRequest {
    request(
        &delta(),
        Capability::FsWrite(exact(r"C:\Users\ala\Dokumenty\notatka.txt")),
        CommandOrigin::Agent,
    )
}

#[tokio::test]
async fn cards_and_tokens_from_before_the_lock_stop_working() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = broker(dir.path(), &clock, &audit);
    set_pin(&b).await;
    // Bez kłódki (L3): token na zapis w profilu bez pytania i karta podniesienia do L4.
    let token = allowed(b.decide(in_profile()).await);
    let raise = AutonomyChangeRequest {
        target: AutonomyTarget::Global,
        level: AutonomyLevel::L4,
        until_ms: None,
        origin: ChangeOrigin::UserInterface,
    };
    let card = b.request_autonomy_change(raise).await.unwrap().unwrap();
    let ch = challenge(&b, card);

    enable(&b).await;
    let needed = token.cap.clone();
    assert!(
        b.verify(&token, &needed, &delta()).is_err(),
        "token sprzed kłódki"
    );
    let late = b
        .resolve(card, ApprovalDecision::Allow, proof(&ch, &clock, false))
        .await;
    assert!(late.is_err(), "karta sprzed kłódki nie podnosi trybu");
    assert_eq!(level(&b), AutonomyLevel::L1);
    // Otwarcie i ponowne zamknięcie — to samo dla tokenów z czasu otwarcia.
    let open = ChildLockChange::Open {
        pin: pin(),
        for_ms: Some(60_000),
        session: None,
    };
    b.change_child_lock(open).await.unwrap();
    let token = allowed(b.decide(in_profile()).await);
    clock.advance(60_000);
    assert!(
        b.verify(&token, &token.cap.clone(), &delta()).is_err(),
        "termin minął"
    );
    assert_eq!(level(&b), AutonomyLevel::L1);
}

#[tokio::test]
async fn card_opened_while_unlocked_cannot_switch_mode_or_grant_after_locking() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = broker(dir.path(), &clock, &audit);
    set_pin(&b).await;
    enable(&b).await;
    let open = ChildLockChange::Open {
        pin: pin(),
        for_ms: None,
        session: None,
    };
    b.change_child_lock(open).await.unwrap();
    // Otwarta kłódka, tryb L3: zapis poza profilem pyta.
    let outside = risky();
    let ticket = needs_approval(b.decide(outside).await);
    b.change_child_lock(ChildLockChange::Close).await.unwrap();
    // Karta wygasła razem z zamknięciem; nowa przy kłódce nie oferuje „zawsze”.
    assert!(b.pending().iter().all(|c| c.request.id != ticket.id));
    let outside = risky();
    let ticket = needs_approval(b.decide(outside).await);
    let ch = challenge(&b, ticket.id);
    assert!(!ch.request.grantable && !ch.request.auto_switch);
    approve(&b, &clock, ticket.id, ApprovalDecision::Allow).await;
}

#[tokio::test]
async fn widening_needs_the_audit_record_first() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = broker(dir.path(), &clock, &audit);
    set_pin(&b).await;
    enable(&b).await;
    audit.set_failing(true);
    let open = ChildLockChange::Open {
        pin: pin(),
        for_ms: None,
        session: None,
    };
    assert!(b.change_child_lock(open).await.is_err());
    let off = ChildLockChange::Enable {
        pin: pin(),
        on: false,
    };
    assert!(b.change_child_lock(off).await.is_err());
    assert_eq!(level(&b), AutonomyLevel::L1, "bez Audytu kłódka zostaje");
    assert!(b.child_lock().enabled);
    // Zawężenie działa i bez Audytu (błąd wraca, stan zamknięty).
    let _ = b.change_child_lock(ChildLockChange::Close).await;
    assert_eq!(level(&b), AutonomyLevel::L1);
}

#[tokio::test]
async fn open_until_exit_closes_when_the_app_disconnects() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = Arc::new(broker(dir.path(), &clock, &audit));
    set_pin(&b).await;
    enable(&b).await;
    let first = CoreSession::new(b.clone());
    let second = CoreSession::new(b.clone());
    let open = ChildLockChange::Open {
        pin: pin(),
        for_ms: None,
        session: None,
    };
    b.change_child_lock(open).await.unwrap();
    assert_eq!(level(&b), AutonomyLevel::L3);
    drop(first);
    assert_eq!(
        level(&b),
        AutonomyLevel::L3,
        "inne połączenie aplikacji trwa"
    );
    drop(second);
    assert_eq!(
        level(&b),
        AutonomyLevel::L1,
        "Alfa zamknięta — kłódka zamknięta"
    );
    assert!(!b.child_lock().tamper_resistant || b.child_lock().enabled);
}

/// Nieudany zapis pliku kłódki po zgodzie: Audyt ma „zgoda”, potem „błąd” — nigdy „ok” (plan 50 Z.7).
#[tokio::test]
async fn failed_save_after_grant_is_never_logged_as_ok() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = broker(dir.path(), &clock, &audit);
    set_pin(&b).await;
    enable(&b).await;
    // Katalog w miejscu pliku tymczasowego — zapis pliku kłódki się nie uda.
    let tmp = dir.path().join(CHILD_LOCK_FILE).with_extension("tmp");
    std::fs::create_dir(&tmp).unwrap();
    let on = ChildLockChange::Transcripts {
        pin: pin(),
        on: true,
    };
    assert!(b.change_child_lock(on).await.is_err());
    assert!(!b.child_lock().transcripts);
    let trail: Vec<serde_json::Value> = audit
        .events()
        .into_iter()
        .map(|e| e.payload)
        .filter(|p| p["op"] == "transcripts_on")
        .collect();
    assert_eq!(
        trail,
        vec![
            serde_json::json!({ "op": "transcripts_on", "stage": "granted" }),
            serde_json::json!({ "op": "transcripts_on", "ok": false }),
        ]
    );
    std::fs::remove_dir(&tmp).unwrap();
    let on = ChildLockChange::Transcripts {
        pin: pin(),
        on: true,
    };
    assert!(b.change_child_lock(on).await.unwrap().transcripts);
    let last = audit.events().last().map(|e| e.payload.clone());
    assert_eq!(
        last,
        Some(serde_json::json!({ "op": "transcripts_on", "ok": true }))
    );
}

/// „Do końca rozmowy” (Z.7): kłódka otwarta tylko dla jednej rozmowy; inne rozmowy, działania bez sesji
/// i zgody stałe mają sufit; zamknięcie i rozłączenie aplikacji wracają do pełnej kłódki.
#[tokio::test]
async fn open_for_one_conversation_only() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = Arc::new(broker(dir.path(), &clock, &audit));
    set_pin(&b).await;
    enable(&b).await;
    let app = CoreSession::new(b.clone());
    let h = delta();
    let other = core_bus_contract::SessionId::from("inna-rozmowa");
    let open = ChildLockChange::Open {
        pin: pin(),
        for_ms: Some(1),
        session: Some(h.session.clone()),
    };
    let view = b.change_child_lock(open).await.unwrap();
    assert!(view.conversation && view.open_until_ms.is_some());
    assert_eq!(level(&b), AutonomyLevel::L3, "ta rozmowa bez sufitu");
    assert_eq!(b.autonomy(&other, None), AutonomyLevel::L1, "inna rozmowa");
    clock.advance(3_600_000);
    assert_eq!(
        level(&b),
        AutonomyLevel::L3,
        "bez terminu — do końca rozmowy"
    );
    let names: Vec<_> = audit.events().into_iter().map(|e| e.payload).collect();
    assert!(
        names
            .iter()
            .any(|p| p["op"] == "open_conversation" && p["ok"] == true)
    );

    // Podniesienie: tylko poziom tej rozmowy (przegląd 5.11, P2); inna rozmowa i tryb globalny — kłódka.
    let raise = |target| AutonomyChangeRequest {
        target,
        level: AutonomyLevel::L4,
        until_ms: None,
        origin: ChangeOrigin::UserInterface,
    };
    let here = AutonomyTarget::Session {
        session: h.session.clone(),
    };
    assert!(b.request_autonomy_change(raise(here)).await.is_ok());
    let there = AutonomyTarget::Session {
        session: other.clone(),
    };
    assert!(b.request_autonomy_change(raise(there)).await.is_err());
    assert!(
        b.request_autonomy_change(raise(AutonomyTarget::Global))
            .await
            .is_err()
    );

    let closed = b.change_child_lock(ChildLockChange::Close).await.unwrap();
    assert!(!closed.conversation && closed.open_until_ms.is_none());
    assert_eq!(level(&b), AutonomyLevel::L1);

    let again = ChildLockChange::Open {
        pin: pin(),
        for_ms: None,
        session: Some(h.session.clone()),
    };
    b.change_child_lock(again).await.unwrap();
    assert_eq!(level(&b), AutonomyLevel::L3);
    drop(app);
    assert_eq!(
        level(&b),
        AutonomyLevel::L1,
        "Alfa zamknięta — kłódka zamknięta"
    );
    assert!(!b.child_lock().conversation);
}
