//! Testy atrapy: kontrakt współdzielony + nagrywanie wywołań, zdarzeń i wstrzykiwanie błędu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_bus_contract::SessionId;
use personas_contract::{
    CastError, ChangeOrigin, EVENT_CAST_CHANGED, EVENT_ROLE_ASSIGNED, PersonaId, Personas,
    PersonasError, RoleId, contract_tests,
};
use personas_fake::FakePersonas;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|| async { FakePersonas::new() }).await;
}

#[tokio::test]
async fn records_calls_and_events() {
    let fake = FakePersonas::new();
    let s = SessionId::new("s1");
    fake.apply_command(&s, "Delta, przejmij weryfikację", ChangeOrigin::Voice)
        .await
        .unwrap()
        .unwrap();
    let mut bad = fake.cast(&s);
    bad.unassign(&PersonaId::alfa(), &RoleId::conductor());
    assert_eq!(
        fake.set_cast(&s, bad, ChangeOrigin::Ui).await.map(|_| ()),
        Err(PersonasError::Cast(CastError::NoConductor))
    );
    let calls = fake.set_cast_calls();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].accepted && !calls[1].accepted);
    assert_eq!(calls[0].origin, ChangeOrigin::Voice);
    let kinds: Vec<String> = fake.events().iter().map(|e| e.kind.to_string()).collect();
    assert_eq!(kinds, [EVENT_CAST_CHANGED, EVENT_ROLE_ASSIGNED]);
    assert_eq!(
        fake.events()[1].agent.as_ref().map(|a| a.as_str()),
        Some("delta")
    );
}

#[tokio::test]
async fn fixtures_and_injected_failure() {
    let fake = FakePersonas::new();
    let s = SessionId::new("s2");
    let solo = personas_contract::Catalog::builtin().solo_cast(Some(&PersonaId::beta()), false);
    fake.preset_cast(&s, solo).unwrap();
    assert_eq!(fake.resolve_addressee(&s, "co słychać"), PersonaId::beta());
    assert!(fake.events().is_empty() && fake.set_cast_calls().is_empty());
    fake.fail_next(PersonasError::NotStarted);
    assert_eq!(
        fake.set_voice(&s, true, ChangeOrigin::System)
            .await
            .map(|_| ()),
        Err(PersonasError::NotStarted)
    );
    assert!(fake.set_voice(&s, true, ChangeOrigin::System).await.is_ok());
}
