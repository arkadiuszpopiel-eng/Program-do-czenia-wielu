//! Z.8 (plan 50 §5): próby obejścia kłódki rodzica w Sędzim — głosem, przez agentkę, przez
//! zapamiętany „Max dostęp” na modelu lokalnym i przez zgadywanie PIN-u w otwarciu. Każda kończy
//! się odmową bez karty w oknie Sędziego; dziecko zostaje na „Ręcznie”.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;

use safety_broker_contract::contract_tests::{
    delta, exact, needs_approval, request, set_level, test_policy,
};
use safety_broker_contract::{
    ApprovalChannel, AutonomyChangeRequest, AutonomyLevel, AutonomyTarget, Broker, BrokerError,
    Capability, ChangeOrigin, ChildLockChange, CommandOrigin, KernelRule, ModelLocation,
    ModelScope, Pin,
};
use safety_broker_impl::audit::MemoryAudit;
use safety_broker_impl::{BrokerEngine, CHILD_LOCK_FILE, PinParams};
use watchdog_contract::ManualClock;

fn broker(dir: &Path, clock: &Arc<ManualClock>) -> BrokerEngine {
    let audit = Arc::new(MemoryAudit::default());
    common::engine_with(test_policy(), clock.clone(), audit)
        .with_standing_level(dir.join("autonomy-standing.json"))
        .with_child_lock(dir.join(CHILD_LOCK_FILE), PinParams::FAST_FOR_TESTS, true)
}

fn pin(s: &str) -> Pin {
    Pin::new(s).unwrap()
}

fn level(b: &BrokerEngine) -> AutonomyLevel {
    let h = delta();
    b.autonomy(&h.session, h.agent.as_ref())
}

async fn locked(b: &BrokerEngine) {
    let set = ChildLockChange::SetPin {
        old: None,
        new: pin("2468"),
    };
    b.change_child_lock(set).await.unwrap();
    let on = ChildLockChange::Enable {
        pin: pin("2468"),
        on: true,
    };
    b.change_child_lock(on).await.unwrap();
}

fn change(target: AutonomyTarget, origin: ChangeOrigin) -> AutonomyChangeRequest {
    AutonomyChangeRequest {
        target,
        level: AutonomyLevel::L4,
        until_ms: None,
        origin,
    }
}

#[tokio::test]
async fn voice_and_agent_cannot_open_the_padlock() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(dir.path(), &clock);
    locked(&b).await;
    let models = AutonomyTarget::Models {
        scope: ModelScope::All,
    };
    for target in [AutonomyTarget::Global, models] {
        let voice = b
            .request_autonomy_change(change(target.clone(), ChangeOrigin::UserVoice))
            .await;
        assert!(voice.is_err(), "głos przy kłódce: {target:?}");
        let agent = b
            .request_autonomy_change(change(target, ChangeOrigin::Agent("delta".into())))
            .await;
        assert_eq!(
            agent,
            Err(BrokerError::KernelBlock(KernelRule::SelfEscalation))
        );
    }
    assert!(
        b.pending().is_empty(),
        "żadnej karty dla rodzica do kliknięcia"
    );
    assert_eq!(level(&b), AutonomyLevel::L1);
}

#[tokio::test]
async fn remembered_local_max_access_does_not_lift_the_padlock() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(dir.path(), &clock);
    let local = AutonomyTarget::Models {
        scope: ModelScope::Local,
    };
    set_level(&b, &clock, local, AutonomyLevel::L4).await;
    locked(&b).await;
    let mut write = request(
        &delta(),
        Capability::FsWrite(exact(r"C:\Users\Ala\Documents\raport.txt")),
        CommandOrigin::Agent,
    );
    write.facts.model_location = Some(ModelLocation::Local);
    needs_approval(b.decide(write).await);
    assert_eq!(b.metrics().max_access, None, "Max dostęp nie obowiązuje");
}

#[tokio::test]
async fn guessing_the_pin_in_open_counts_and_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(dir.path(), &clock);
    locked(&b).await;
    for guess in ["0000", "1111", "1234", "9999", "2469"] {
        let open = ChildLockChange::Open {
            pin: pin(guess),
            for_ms: None,
        };
        assert!(b.change_child_lock(open).await.is_err());
    }
    // Po serii — nawet dobry PIN nie otwiera do końca blokady.
    let open = ChildLockChange::Open {
        pin: pin("2468"),
        for_ms: None,
    };
    assert!(b.change_child_lock(open).await.is_err());
    let view = b.child_lock();
    assert!(view.blocked_until_ms.is_some() && view.enabled);
    assert_eq!(level(&b), AutonomyLevel::L1);
    // Zamknięcie i obniżenie działają zawsze (bez PIN-u).
    assert!(b.change_child_lock(ChildLockChange::Close).await.is_ok());
}

/// Z.7 (decyzja właściciela 3.11): pełną treść rozmów w dzienniku włącza tylko rodzic PIN-em;
/// przełącznik w zapisie Sędziego (przetrwa restart), w Audycie rodzaj zmiany bez PIN-u.
#[tokio::test]
async fn transcripts_switch_needs_the_parent_pin() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = common::engine_with(test_policy(), clock.clone(), audit.clone()).with_child_lock(
        dir.path().join(CHILD_LOCK_FILE),
        PinParams::FAST_FOR_TESTS,
        true,
    );
    locked(&b).await;
    assert!(!b.child_lock().transcripts, "domyślnie tylko tematy");
    let wrong = ChildLockChange::Transcripts {
        pin: pin("1357"),
        on: true,
    };
    assert!(b.change_child_lock(wrong).await.is_err());
    assert!(!b.child_lock().transcripts);
    assert_eq!(b.child_lock().attempts_left, 4, "zła próba liczona");
    let on = ChildLockChange::Transcripts {
        pin: pin("2468"),
        on: true,
    };
    assert!(b.change_child_lock(on).await.unwrap().transcripts);
    let text = format!("{:?}", audit.events());
    assert!(text.contains("transcripts_on") && !text.contains("2468"));
    drop(b);
    let b = broker(dir.path(), &clock);
    assert!(b.child_lock().transcripts, "przetrwa restart Sędziego");
    // Bez Audytu nie zmienia się w żadną stronę.
    let audit = Arc::new(MemoryAudit::default());
    let b2 = common::engine_with(test_policy(), clock.clone(), audit.clone()).with_child_lock(
        dir.path().join(CHILD_LOCK_FILE),
        PinParams::FAST_FOR_TESTS,
        true,
    );
    drop(b);
    audit.set_failing(true);
    let off = ChildLockChange::Transcripts {
        pin: pin("2468"),
        on: false,
    };
    assert!(b2.change_child_lock(off).await.is_err());
    assert!(b2.child_lock().transcripts);
}
