//! Testy implementacji: kontrakt współdzielony, manifest, cykl życia, zdarzenia na magistrali.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use core_bus_contract::SessionId;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use personas_contract::{
    Catalog, ChangeOrigin, EVENT_CAST_CHANGED, EVENT_PERSONA_ADDED, EVENT_ROLE_ASSIGNED, NameForms,
    PersonaId, Personas, PersonasError, TemplateId, builtin_personas, contract_tests, event_kind,
};
use personas_impl::{MODULE_TOML, PersonasModule};

async fn started() -> (PersonasModule, FakeBus) {
    let bus = FakeBus::default();
    let mut module = PersonasModule::new().unwrap();
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    (module, bus)
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|| async { started().await.0 }).await;
}

#[test]
fn manifest_is_valid() {
    let module = PersonasModule::new().unwrap();
    let m = module.manifest();
    assert_eq!(m.id.as_str(), "personas");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.lifecycle, Lifecycle::Always);
    assert_eq!(m.provides[0].to_string(), "personas-contract@1");
    assert!(MODULE_TOML.contains("core-bus-contract@1"));
}

#[tokio::test]
async fn lifecycle_and_not_started() {
    let mut module = PersonasModule::new().unwrap();
    let s = SessionId::new("s");
    assert_eq!(module.health(), HealthStatus::NotStarted);
    // Odczyt działa bez magistrali, zmiany nie (muszą trafić do dziennika).
    assert_eq!(module.cast(&s).conductor(), Some(PersonaId::alfa()));
    let err = module
        .apply_command(&s, "Beta, teraz ty prowadzisz", ChangeOrigin::Voice)
        .await;
    assert_eq!(err, Err(PersonasError::NotStarted));
    assert_eq!(module.cast(&s).conductor(), Some(PersonaId::alfa()));
    assert_eq!(module.stop().await, Err(ModuleError::NotStarted));
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(FakeBus::default()));
    module.start(ctx.clone()).await.unwrap();
    assert_eq!(module.health(), HealthStatus::Healthy);
    assert_eq!(module.start(ctx).await, Err(ModuleError::AlreadyStarted));
    module.stop().await.unwrap();
    assert_eq!(module.health(), HealthStatus::NotStarted);
}

#[tokio::test]
async fn cast_change_published_on_bus() {
    let (module, bus) = started().await;
    let s = SessionId::new("s1");
    module
        .apply_command(&s, "Delta, przejmij weryfikację", ChangeOrigin::Voice)
        .await
        .unwrap()
        .unwrap();
    let changed = bus.recorded_of_kind(&event_kind(EVENT_CAST_CHANGED));
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].session.as_ref().map(|s| s.as_str()), Some("s1"));
    assert_eq!(changed[0].payload["origin"], "voice");
    assert_eq!(changed[0].payload["removed"][0][1], "critic");
    assert_eq!(
        changed[0].payload["warnings"][0]["warning"],
        "critic_also_author"
    );
    let assigned = bus.recorded_of_kind(&event_kind(EVENT_ROLE_ASSIGNED));
    assert_eq!(assigned.len(), 1);
    assert_eq!(assigned[0].payload["persona"], "delta");
    // Nie-polecenie nic nie publikuje.
    assert_eq!(
        module
            .apply_command(&s, "Delta, sprawdź pogodę", ChangeOrigin::Voice)
            .await
            .unwrap(),
        None
    );
    assert_eq!(bus.recorded().len(), 2);
}

#[tokio::test]
async fn creator_and_import_export() {
    let (module, bus) = started().await;
    let mut epsilon = builtin_personas()[1].clone();
    epsilon.id = PersonaId::new("epsilon");
    epsilon.name = "Epsilon".into();
    epsilon.glyph = 'ε';
    epsilon.wake_phrases = vec!["Hej Epsilon".into()];
    epsilon.forms = NameForms::uniform("Epsilon");
    epsilon.builtin = false;
    module.add_persona(epsilon).await.unwrap();
    assert_eq!(
        bus.recorded_of_kind(&event_kind(EVENT_PERSONA_ADDED)).len(),
        1
    );
    let s = SessionId::new("s-e");
    module
        .apply_command(&s, "Epsilon, teraz ty prowadzisz", ChangeOrigin::Text)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        module.resolve_addressee(&s, "co teraz"),
        PersonaId::new("epsilon")
    );
    let export = module.export();

    let (other, other_bus) = started().await;
    let changes = other.import(&export).await.unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(other.cast(&s).conductor(), Some(PersonaId::new("epsilon")));
    assert!(
        !other_bus
            .recorded_of_kind(&event_kind(EVENT_CAST_CHANGED))
            .is_empty()
    );
}

#[tokio::test]
async fn default_template_and_prompt_template() {
    let module = PersonasModule::with_catalog(Catalog::builtin()).unwrap();
    module.set_default_template(&TemplateId::coding()).unwrap();
    let s = SessionId::new("s");
    assert_eq!(module.cast(&s).conductor(), Some(PersonaId::delta()));
    assert!(
        module
            .set_default_template(&TemplateId::new("brak"))
            .is_err()
    );
    assert!(module.set_prompt_template(Some("{imie}".into())).is_err());
    module
        .set_prompt_template(Some("{imie}: {role}\n{zasady}".into()))
        .unwrap();
    let prompt = module.system_prompt(&s, &PersonaId::delta()).unwrap();
    assert!(prompt.starts_with("Delta: Dyrygentka, Koderka"), "{prompt}");
}
