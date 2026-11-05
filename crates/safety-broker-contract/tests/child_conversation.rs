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
