//! Atrapa modułu `personas` (docs/modules/personas/SPEC.md „Fake”): persony, role i obsady
//! z fixture'ów (katalogu), zapis wywołań `set_cast`, nagrane zdarzenia zamiast magistrali,
//! wstrzykiwanie błędu. Logika (walidacja, parser, adresowanie) jest wspólna z `-impl`
//! (`PersonasState` z kontraktu), więc atrapa przechodzi ten sam test kontraktowy.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::{Event, SessionId};
use personas_contract::{
    Cast, CastChange, CastTemplate, Catalog, ChangeOrigin, Persona, PersonaId, Personas,
    PersonasError, PersonasExport, PersonasState, Role,
};

/// Zapis jednego wywołania zmiany obsady.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCastCall {
    /// Sesja.
    pub session: SessionId,
    /// Żądana obsada (także odrzucona).
    pub cast: Cast,
    /// Źródło.
    pub origin: ChangeOrigin,
    /// Czy została przyjęta.
    pub accepted: bool,
}

#[derive(Default)]
struct Inner {
    state: PersonasState,
    events: Vec<Event>,
    calls: Vec<SetCastCall>,
    fail_next: Option<PersonasError>,
}

/// Deterministyczna usługa person bez magistrali.
#[derive(Default)]
pub struct FakePersonas {
    inner: Mutex<Inner>,
}

impl FakePersonas {
    /// Atrapa z katalogiem wbudowanym.
    pub fn new() -> Self {
        Self::with_catalog(Catalog::builtin())
    }

    /// Atrapa z własnym katalogiem (fixture).
    pub fn with_catalog(catalog: Catalog) -> Self {
        Self {
            inner: Mutex::new(Inner {
                state: PersonasState::new(catalog),
                ..Inner::default()
            }),
        }
    }

    /// Ustawia obsadę sesji bez zdarzeń i bez zapisu wywołania (przygotowanie testu).
    pub fn preset_cast(&self, session: &SessionId, cast: Cast) -> Result<(), PersonasError> {
        self.lock()
            .state
            .set_cast(session, cast, ChangeOrigin::System)
            .map(|_| ())
    }

    /// Zdarzenia, które `-impl` opublikowałby na magistrali.
    pub fn events(&self) -> Vec<Event> {
        self.lock().events.clone()
    }

    /// Wszystkie wywołania zmiany obsady (przez `set_cast`, `set_voice`, `apply_command`).
    pub fn set_cast_calls(&self) -> Vec<SetCastCall> {
        self.lock().calls.clone()
    }

    /// Następna operacja zmieniająca stan zwróci ten błąd (jednorazowo).
    pub fn fail_next(&self, error: PersonasError) {
        self.lock().fail_next = Some(error);
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn record(
        inner: &mut Inner,
        session: &SessionId,
        requested: Cast,
        origin: ChangeOrigin,
        result: Result<CastChange, PersonasError>,
    ) -> Result<CastChange, PersonasError> {
        inner.calls.push(SetCastCall {
            session: session.clone(),
            cast: result.as_ref().map_or(requested, |c| c.after.clone()),
            origin,
            accepted: result.is_ok(),
        });
        if let Ok(change) = &result {
            inner.events.extend(change.events());
        }
        result
    }
}

#[async_trait]
impl Personas for FakePersonas {
    fn personas(&self) -> Vec<Persona> {
        self.lock().state.catalog().personas().to_vec()
    }

    fn roles(&self) -> Vec<Role> {
        self.lock().state.catalog().roles().to_vec()
    }

    fn templates(&self) -> Vec<CastTemplate> {
        self.lock().state.catalog().templates().to_vec()
    }

    fn cast(&self, session: &SessionId) -> Cast {
        self.lock().state.cast(session)
    }

    async fn set_cast(
        &self,
        session: &SessionId,
        cast: Cast,
        origin: ChangeOrigin,
    ) -> Result<CastChange, PersonasError> {
        let mut inner = self.lock();
        if let Some(err) = inner.fail_next.take() {
            return Err(err);
        }
        let result = inner.state.set_cast(session, cast.clone(), origin);
        Self::record(&mut inner, session, cast, origin, result)
    }

    async fn set_voice(
        &self,
        session: &SessionId,
        voice: bool,
        origin: ChangeOrigin,
    ) -> Result<CastChange, PersonasError> {
        let mut inner = self.lock();
        if let Some(err) = inner.fail_next.take() {
            return Err(err);
        }
        let requested = inner.state.cast(session);
        let result = inner.state.set_voice(session, voice, origin);
        Self::record(&mut inner, session, requested, origin, result)
    }

    async fn apply_command(
        &self,
        session: &SessionId,
        text: &str,
        origin: ChangeOrigin,
    ) -> Result<Option<CastChange>, PersonasError> {
        let mut inner = self.lock();
        if let Some(err) = inner.fail_next.take() {
            return Err(err);
        }
        let requested = inner.state.cast(session);
        match inner.state.apply_command(session, text, origin) {
            Ok(None) => Ok(None),
            Ok(Some(change)) => {
                Self::record(&mut inner, session, requested, origin, Ok(change)).map(Some)
            }
            Err(err) => Self::record(&mut inner, session, requested, origin, Err(err)).map(Some),
        }
    }

    fn resolve_addressee(&self, session: &SessionId, text: &str) -> PersonaId {
        self.lock().state.resolve_addressee(session, text)
    }

    fn system_prompt(
        &self,
        session: &SessionId,
        persona: &PersonaId,
    ) -> Result<String, PersonasError> {
        self.lock().state.system_prompt(session, persona)
    }

    async fn add_persona(&self, persona: Persona) -> Result<(), PersonasError> {
        let mut inner = self.lock();
        if let Some(err) = inner.fail_next.take() {
            return Err(err);
        }
        let event = inner.state.add_persona(persona)?;
        inner.events.push(event);
        Ok(())
    }

    async fn add_role(&self, role: Role) -> Result<(), PersonasError> {
        self.lock().state.add_role(role)
    }

    async fn add_template(&self, template: CastTemplate) -> Result<(), PersonasError> {
        self.lock().state.add_template(template)
    }

    fn export(&self) -> PersonasExport {
        self.lock().state.export()
    }
}
