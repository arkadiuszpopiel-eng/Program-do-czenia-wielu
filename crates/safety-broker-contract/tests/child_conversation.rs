//! Kłódka profilu Dziecko otwarta „do końca rozmowy” (plan 50 Z.7): sufit znika tylko dla tej sesji.

use core_bus_contract::SessionId;
use safety_broker_contract::{
    AutonomyEntry, AutonomyLevel, AutonomyTable, AutonomyTarget, ChildCeiling, OPEN_UNTIL_CLOSE,
};

fn entry(level: AutonomyLevel) -> AutonomyEntry {
    AutonomyEntry {
        level,
        until_ms: None,
    }
}

#[test]
fn lock_open_for_one_conversation_caps_every_other() {
    let (a, b) = (SessionId::from("a"), SessionId::from("b"));
    let mut t = AutonomyTable::default();
    t.set(AutonomyTarget::Global, entry(AutonomyLevel::L4));
    let open = ChildCeiling {
        open_until_ms: Some(OPEN_UNTIL_CLOSE),
        ..ChildCeiling::locked()
    };
    t.set_child_for(open, a.clone());
    assert_eq!(t.effective(&a, None, 10), AutonomyLevel::L4);
    assert_eq!(t.effective(&b, None, 10), AutonomyLevel::L1);
    assert_eq!(t.ceiling(10), Some(AutonomyLevel::L1), "bez sesji — sufit");
    t.set_child(Some(ChildCeiling::locked()));
    assert_eq!(t.child_session(), None);
    assert_eq!(t.effective(&a, None, 10), AutonomyLevel::L1);
}

/// Max dostęp nie obowiązuje przy kłódce otwartej tylko dla jednej rozmowy (przegląd 5.11, P1).
#[test]
fn max_access_stays_off_in_the_open_conversation() {
    use safety_broker_contract::{ModelLocation, ModelScope};
    let a = SessionId::from("a");
    let mut t = AutonomyTable::default();
    t.set(
        AutonomyTarget::Models {
            scope: ModelScope::All,
        },
        entry(AutonomyLevel::L4),
    );
    let open = ChildCeiling {
        open_until_ms: Some(OPEN_UNTIL_CLOSE),
        ..ChildCeiling::locked()
    };
    t.set_child_for(open, a.clone());
    let level = t.effective_for(&a, None, Some(ModelLocation::Local), 10);
    assert_eq!(level, AutonomyLevel::L3, "poziom rozmowy bez Max dostępu");
    t.set_child(None);
    let level = t.effective_for(&a, None, Some(ModelLocation::Local), 10);
    assert_eq!(level, AutonomyLevel::L4);
}

/// Podniesienie rozmowy w czasie otwarcia nie wraca po zamknięciu przy otwarciu na czas (przegląd 6.11, P-1).
#[test]
fn raise_made_during_conversation_opening_does_not_come_back() {
    let a = SessionId::from("a");
    let mut t = AutonomyTable::default();
    t.set(AutonomyTarget::Global, entry(AutonomyLevel::L2));
    let open = ChildCeiling {
        open_until_ms: Some(OPEN_UNTIL_CLOSE),
        ..ChildCeiling::locked()
    };
    t.set_child_for(open, a.clone());
    t.set(
        AutonomyTarget::Session { session: a.clone() },
        entry(AutonomyLevel::L4),
    );
    assert_eq!(t.effective(&a, None, 10), AutonomyLevel::L4);
    t.set_child(Some(ChildCeiling::locked()));
    t.set_child(Some(ChildCeiling {
        open_until_ms: Some(1_000),
        ..ChildCeiling::locked()
    }));
    assert_eq!(
        t.effective(&a, None, 10),
        AutonomyLevel::L2,
        "poziom globalny, nie L4"
    );
}
