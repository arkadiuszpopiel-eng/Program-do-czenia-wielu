//! Tabela poziomów autonomii: kolejność rozstrzygania, terminy, Max dostęp, pochodzenie modelu
//! (przeniesione z testów jednostkowych `autonomy.rs` — limit 400 linii pliku).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_bus_contract::{AgentId, SessionId};
use safety_broker_contract::*;

fn entry(level: AutonomyLevel) -> AutonomyEntry {
    AutonomyEntry {
        level,
        until_ms: None,
    }
}

#[test]
fn resolution_order_and_expiry() {
    let s = SessionId::new("s1");
    let a = AgentId::new("delta");
    let mut t = AutonomyTable::default();
    assert_eq!(t.effective(&s, Some(&a), 0), AutonomyLevel::L3);
    t.set(AutonomyTarget::Global, entry(AutonomyLevel::L4));
    assert_eq!(t.effective(&s, Some(&a), 0), AutonomyLevel::L4);
    t.set(
        AutonomyTarget::Session { session: s.clone() },
        entry(AutonomyLevel::L2),
    );
    t.set(
        AutonomyTarget::Agent { agent: a.clone() },
        entry(AutonomyLevel::L4),
    );
    assert_eq!(t.effective(&s, Some(&a), 0), AutonomyLevel::L2);
    t.set(
        AutonomyTarget::SessionAgent {
            session: s.clone(),
            agent: a.clone(),
        },
        AutonomyEntry {
            level: AutonomyLevel::L4,
            until_ms: Some(100),
        },
    );
    assert_eq!(t.effective(&s, Some(&a), 99), AutonomyLevel::L4);
    assert_eq!(t.effective(&s, Some(&a), 100), AutonomyLevel::L2);
    assert_eq!(t.effective(&s, None, 0), AutonomyLevel::L2);
    assert!(t.clear(&AutonomyTarget::Session { session: s.clone() }));
    assert_eq!(t.effective(&s, Some(&a), 200), AutonomyLevel::L4);
    assert_eq!(t.entries().len(), 3);
}

#[test]
fn max_access_covers_model_locations() {
    let s = SessionId::new("s1");
    let a = AgentId::new("delta");
    let local = Some(ModelLocation::Local);
    let cloud = Some(ModelLocation::Cloud);
    let mut t = AutonomyTable::default();
    t.set(
        AutonomyTarget::Session { session: s.clone() },
        entry(AutonomyLevel::L1),
    );
    assert_eq!(t.effective_for(&s, Some(&a), local, 0), AutonomyLevel::L1);
    let models = |scope| AutonomyTarget::Models { scope };
    t.set(models(ModelScope::Local), entry(AutonomyLevel::L4));
    assert_eq!(t.effective_for(&s, Some(&a), local, 0), AutonomyLevel::L4);
    assert_eq!(t.effective_for(&s, Some(&a), cloud, 0), AutonomyLevel::L1);
    assert_eq!(t.effective_for(&s, Some(&a), None, 0), AutonomyLevel::L1);
    // Jeden przełącznik: „chmura” zastępuje „lokalne”.
    t.set(models(ModelScope::Cloud), entry(AutonomyLevel::L4));
    assert_eq!(
        t.max_access(0),
        Some((ModelScope::Cloud, AutonomyLevel::L4))
    );
    assert_eq!(t.effective_for(&s, Some(&a), local, 0), AutonomyLevel::L1);
    assert_eq!(t.effective_for(&s, Some(&a), cloud, 0), AutonomyLevel::L4);
    // Działanie bez modelu (wtyczka, zrzut ekranu) — tylko „wszystkie”.
    assert_eq!(t.effective_for(&s, Some(&a), None, 0), AutonomyLevel::L1);
    t.set(models(ModelScope::All), entry(AutonomyLevel::L4));
    assert_eq!(t.effective_for(&s, Some(&a), local, 0), AutonomyLevel::L4);
    assert_eq!(t.effective_for(&s, Some(&a), None, 0), AutonomyLevel::L4);
    assert_eq!(t.entries().len(), 2);
    // Wyłączenie: poziom domyślny albo niższy usuwa wpis.
    t.set(models(ModelScope::All), entry(AutonomyLevel::L3));
    assert_eq!(t.max_access(0), None);
    assert_eq!(t.effective(&s, Some(&a), 0), AutonomyLevel::L1);
    assert_eq!(t.effective_for(&s, Some(&a), local, 0), AutonomyLevel::L1);
}

#[test]
fn model_location_from_router_ids() {
    assert_eq!(
        ModelLocation::of_model("local:bielik-4.5b"),
        ModelLocation::Local
    );
    assert_eq!(ModelLocation::of_model("LOCAL"), ModelLocation::Local);
    assert_eq!(
        ModelLocation::of_model("anthropic:claude"),
        ModelLocation::Cloud
    );
    assert_eq!(ModelLocation::of_model("auto"), ModelLocation::Cloud);
    assert_eq!(ModelLocation::of_model("localhost:x"), ModelLocation::Cloud);
}
