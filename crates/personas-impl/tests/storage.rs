//! Obsada: trwały zapis przed potwierdzeniem, walidacja odtworzenia i brak zmian po błędzie.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use core_bus_contract::SessionId;
use core_bus_fake::FakeBus;
use core_registry_contract::{Module, ModuleContext};
use personas_contract::{Cast, CastStore, ChangeOrigin, PersonaId, Personas, PersonasError};
use personas_impl::PersonasModule;

#[derive(Default)]
struct Store {
    casts: Mutex<BTreeMap<SessionId, Cast>>,
    fail: AtomicBool,
}

impl CastStore for Store {
    fn load(&self) -> Result<BTreeMap<SessionId, Cast>, String> {
        Ok(self.casts.lock().unwrap().clone())
    }

    fn save(&self, session: &SessionId, cast: &Cast) -> Result<(), String> {
        if self.fail.load(Ordering::SeqCst) {
            return Err("symulowany brak zapisu".into());
        }
        self.casts
            .lock()
            .unwrap()
            .insert(session.clone(), cast.clone());
        Ok(())
    }
}

async fn started() -> (PersonasModule, FakeBus) {
    let bus = FakeBus::default();
    let mut module = PersonasModule::new().unwrap();
    module
        .start(ModuleContext::new(
            module.manifest().id.clone(),
            Arc::new(bus.clone()),
        ))
        .await
        .unwrap();
    (module, bus)
}

#[tokio::test]
async fn failed_save_preserves_cast_and_does_not_publish_a_change() {
    let (module, bus) = started().await;
    let store = Arc::new(Store::default());
    module.bind_store(store.clone()).unwrap();
    let session = SessionId::new("s");
    let before = module.cast(&session);
    store.fail.store(true, Ordering::SeqCst);
    let error = module
        .apply_command(&session, "Delta, przejmij weryfikację", ChangeOrigin::Ui)
        .await
        .unwrap_err();
    assert!(matches!(error, PersonasError::Storage(_)));
    assert_eq!(module.cast(&session), before);
    assert!(
        module
            .set_voice(&session, true, ChangeOrigin::Ui)
            .await
            .is_err()
    );
    assert_eq!(module.cast(&session), before);
    assert!(store.casts.lock().unwrap().is_empty());
    assert!(bus.recorded().is_empty());
}

#[tokio::test]
async fn restart_restores_latest_cast_without_republishing_or_changing_voice() {
    let store = Arc::new(Store::default());
    let session = SessionId::new("s");
    let (module, _) = started().await;
    module.bind_store(store.clone()).unwrap();
    module
        .apply_command(&session, "Delta, przejmij weryfikację", ChangeOrigin::Ui)
        .await
        .unwrap();
    module
        .set_voice(&session, true, ChangeOrigin::Ui)
        .await
        .unwrap();
    let expected = module.cast(&session);
    assert!(
        expected.voice,
        "flaga obsady nie jest uruchomieniem mikrofonu"
    );
    let (restarted, bus) = started().await;
    restarted.bind_store(store).unwrap();
    assert_eq!(restarted.cast(&session), expected);
    assert!(
        bus.recorded().is_empty(),
        "odczyt nie jest nowym poleceniem właściciela"
    );
    restarted.forget_session(&session);
    assert_ne!(
        restarted.cast(&session),
        expected,
        "usunięta sesja nie zostaje w RAM"
    );
}

#[tokio::test]
async fn invalid_restored_cast_is_rejected_without_partial_state_or_binding() {
    let store = Arc::new(Store::default());
    let (module, bus) = started().await;
    let session = SessionId::new("s");
    let before = module.cast(&session);
    let valid = SessionId::new("a-valid");
    store.casts.lock().unwrap().insert(
        valid.clone(),
        Cast::solo(
            PersonaId::new("beta"),
            module.roles().into_iter().map(|r| r.id),
            false,
        ),
    );
    let mut bad = before.clone();
    bad.assignments.insert(
        PersonaId::new("nieistniejaca"),
        before.roles_of(&PersonaId::alfa()),
    );
    store.casts.lock().unwrap().insert(session.clone(), bad);
    assert!(module.bind_store(store).is_err());
    assert_eq!(
        module.cast(&valid),
        before,
        "pierwszy poprawny wpis też nie zostaje po błędzie"
    );
    assert_eq!(module.cast(&session), before);
    assert!(bus.recorded().is_empty());
    module.bind_store(Arc::new(Store::default())).unwrap();
}
