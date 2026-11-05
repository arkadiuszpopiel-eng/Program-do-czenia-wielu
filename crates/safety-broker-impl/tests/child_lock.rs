//! Profil Dziecko (plan 50 §4.4, Z.6): kłódka sprawdzana w Sędzim. Bez PIN-u rodzica najwyżej
//! „Ręcznie” — także gdy zapamiętany jest „Pełny dostęp” albo „Max dostęp”; bez zgód stałych
//! i bez przełączenia z okna zgody. PIN: tylko skrót, limit prób, nigdy w Audycie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;

use safety_broker_contract::contract_tests::{
    challenge, delta, exact, needs_approval, request, set_level, test_policy,
};
use safety_broker_contract::{
    ApprovalChannel, AutonomyChangeRequest, AutonomyLevel, AutonomyTarget, Broker, BrokerError,
    Capability, ChangeOrigin, ChildLockChange, CommandOrigin, ModelScope, PIN_MAX_FAILURES, Pin,
};
use safety_broker_impl::audit::MemoryAudit;
use safety_broker_impl::{BrokerEngine, CHILD_LOCK_FILE, PinParams};
use watchdog_contract::ManualClock;

fn broker(dir: &Path, clock: &Arc<ManualClock>, audit: &Arc<MemoryAudit>) -> BrokerEngine {
    common::engine_with(test_policy(), clock.clone(), audit.clone())
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

async fn raise(
    b: &BrokerEngine,
    target: AutonomyTarget,
    level: AutonomyLevel,
) -> Result<(), BrokerError> {
    let req = AutonomyChangeRequest {
        target,
        level,
        until_ms: None,
        origin: ChangeOrigin::UserInterface,
    };
    b.request_autonomy_change(req).await.map(|_| ())
}

async fn enable(b: &BrokerEngine) {
    b.change_child_lock(ChildLockChange::SetPin {
        old: None,
        new: pin("2468"),
    })
    .await
    .unwrap();
    let view = b
        .change_child_lock(ChildLockChange::Enable {
            pin: pin("2468"),
            on: true,
        })
        .await
        .unwrap();
    assert!(view.enabled && view.pin_set);
}

#[tokio::test]
async fn locked_profile_caps_every_level_even_remembered_full_access() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = broker(dir.path(), &clock, &audit);
    let err = b
        .change_child_lock(ChildLockChange::Enable {
            pin: pin("2468"),
            on: true,
        })
        .await;
    assert!(err.is_err(), "bez ustawionego PIN-u nie ma profilu");
    set_level(&b, &clock, AutonomyTarget::Global, AutonomyLevel::L4).await;
    set_level(
        &b,
        &clock,
        AutonomyTarget::Models {
            scope: ModelScope::All,
        },
        AutonomyLevel::L4,
    )
    .await;
    assert_eq!(level(&b), AutonomyLevel::L4);
    enable(&b).await;
    assert_eq!(
        level(&b),
        AutonomyLevel::L1,
        "kłódka obejmuje zapamiętany Pełny dostęp"
    );

    // Restart Sędziego: profil i sufit wracają z pliku.
    drop(b);
    let b = broker(dir.path(), &clock, &audit);
    assert_eq!(level(&b), AutonomyLevel::L1);
    // Obniżenie zapamiętanego poziomu wolno (zawęża); każde podniesienie — tylko z PIN-em.
    raise(&b, AutonomyTarget::Global, AutonomyLevel::L1)
        .await
        .unwrap();
    for (target, lvl) in [
        (AutonomyTarget::Global, AutonomyLevel::L2),
        (AutonomyTarget::Global, AutonomyLevel::L4),
        (
            AutonomyTarget::Session {
                session: delta().session,
            },
            AutonomyLevel::L3,
        ),
        (
            AutonomyTarget::Models {
                scope: ModelScope::Local,
            },
            AutonomyLevel::L4,
        ),
    ] {
        let refused = raise(&b, target.clone(), lvl).await;
        assert!(
            matches!(refused, Err(BrokerError::InvalidRequest(_))),
            "{target:?} {lvl:?}"
        );
    }
    assert!(
        b.pending().is_empty(),
        "żadna karta podniesienia nie powstaje"
    );
    // Obniżenie (Plan) wolno zawsze.
    raise(
        &b,
        AutonomyTarget::Session {
            session: delta().session,
        },
        AutonomyLevel::L0,
    )
    .await
    .unwrap();
    assert_eq!(level(&b), AutonomyLevel::L0);

    // Zamknięta kłódka: pytanie bez zgód stałych i bez przełączenia z okna.
    raise(
        &b,
        AutonomyTarget::Session {
            session: delta().session,
        },
        AutonomyLevel::L1,
    )
    .await
    .ok();
    let write = request(
        &delta(),
        Capability::FsWrite(exact(r"C:\Users\Ala\Documents\raport.txt")),
        CommandOrigin::Agent,
    );
    let ticket = needs_approval(b.decide(write).await);
    let ch = challenge(&b, ticket.id);
    assert!(!ch.request.grantable && !ch.request.auto_switch);
}

#[tokio::test]
async fn parent_pin_opens_for_a_time_and_closes_by_itself() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = broker(dir.path(), &clock, &audit);
    set_level(&b, &clock, AutonomyTarget::Global, AutonomyLevel::L4).await;
    enable(&b).await;
    let open = |p: &str, for_ms| ChildLockChange::Open {
        pin: pin(p),
        for_ms,
    };
    assert!(
        b.change_child_lock(open("1357", Some(3_600_000)))
            .await
            .is_err()
    );
    assert!(b.change_child_lock(open("2468", Some(0))).await.is_err());
    assert_eq!(level(&b), AutonomyLevel::L1);
    let view = b
        .change_child_lock(open("2468", Some(3_600_000)))
        .await
        .unwrap();
    assert_eq!(view.open_until_ms, Some(4_600_000));
    assert_eq!(
        level(&b),
        AutonomyLevel::L4,
        "otwarta kłódka — tryb rodzica"
    );
    clock.advance(3_600_000);
    assert_eq!(level(&b), AutonomyLevel::L1, "po godzinie zamyka się sama");
    assert_eq!(b.child_lock().open_until_ms, None);

    // „Do zamknięcia Alfy”: do restartu Sędziego; „Zamknij” — od razu, bez PIN-u.
    b.change_child_lock(open("2468", None)).await.unwrap();
    assert_eq!(level(&b), AutonomyLevel::L4);
    b.change_child_lock(ChildLockChange::Close).await.unwrap();
    assert_eq!(level(&b), AutonomyLevel::L1);
    b.change_child_lock(open("2468", None)).await.unwrap();
    drop(b);
    let b = broker(dir.path(), &clock, &audit);
    assert_eq!(level(&b), AutonomyLevel::L1, "restart zamyka kłódkę");

    // Wyłączenie profilu tylko PIN-em; zmiana PIN-u wymaga starego.
    let change = |old: Option<&str>, new: &str| ChildLockChange::SetPin {
        old: old.map(pin),
        new: pin(new),
    };
    assert!(b.change_child_lock(change(None, "1111")).await.is_err());
    b.change_child_lock(change(Some("2468"), "97531"))
        .await
        .unwrap();
    let off = b
        .change_child_lock(ChildLockChange::Enable {
            pin: pin("97531"),
            on: false,
        })
        .await
        .unwrap();
    assert!(!off.enabled);
    assert_eq!(level(&b), AutonomyLevel::L4);

    // Tylko ładunki: identyfikatory i skróty łańcucha mogą losowo zawierać te cyfry.
    let payloads: Vec<_> = audit.events().into_iter().map(|e| e.payload).collect();
    let text = serde_json::to_string(&payloads).unwrap();
    for secret in ["2468", "1357", "97531", "1111"] {
        assert!(!text.contains(secret), "PIN {secret} w Audycie");
    }
    assert!(audit.names().iter().any(|n| n.contains("child")));
}

#[tokio::test]
async fn guessing_the_pin_blocks_further_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = broker(dir.path(), &clock, &audit);
    enable(&b).await;
    for n in 0..PIN_MAX_FAILURES {
        let guess = format!("{:04}", 1000 + n);
        let r = b
            .change_child_lock(ChildLockChange::Open {
                pin: pin(&guess),
                for_ms: None,
            })
            .await;
        assert!(r.is_err());
    }
    let view = b.child_lock();
    assert!(view.blocked_until_ms.is_some_and(|u| u > 1_000_000));
    // Sprawdzenie PIN-u (dziennik rodzica) podlega tej samej blokadzie.
    let verify = ChildLockChange::Verify { pin: pin("2468") };
    assert!(b.change_child_lock(verify).await.is_err());
    // W blokadzie nawet dobry PIN nie otwiera; po restarcie blokada trwa.
    let good = ChildLockChange::Open {
        pin: pin("2468"),
        for_ms: None,
    };
    assert!(b.change_child_lock(good.clone()).await.is_err());
    drop(b);
    let b = broker(dir.path(), &clock, &audit);
    assert!(b.change_child_lock(good.clone()).await.is_err());
    clock.advance(16 * 60_000);
    b.change_child_lock(good).await.unwrap();
    assert_eq!(b.child_lock().attempts_left, PIN_MAX_FAILURES);
    let wrong = ChildLockChange::Verify { pin: pin("9999") };
    assert!(b.change_child_lock(wrong).await.is_err());
    assert_eq!(
        b.child_lock().attempts_left,
        PIN_MAX_FAILURES - 1,
        "zły PIN liczy próbę"
    );
    let right = ChildLockChange::Verify { pin: pin("2468") };
    let view = b.change_child_lock(right).await.unwrap();
    assert_eq!(view.attempts_left, PIN_MAX_FAILURES);
    assert!(
        view.open_until_ms.is_some(),
        "sprawdzenie nie zmienia kłódki"
    );
}
