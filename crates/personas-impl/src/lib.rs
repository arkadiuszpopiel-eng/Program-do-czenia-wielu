//! Implementacja modułu `personas` (docs/modules/personas/SPEC.md, docs/PERSONAS.md).
//!
//! Rdzeń decyzyjny (katalog, walidacja, parser poleceń, adresowanie) to `PersonasState`
//! z kontraktu; ten crate dodaje cykl życia modułu i publikację zdarzeń na magistralę.
//! Zmiany obsady wymagają uruchomionego modułu — każda musi trafić do dziennika.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, SessionId};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use personas_contract::{
    Cast, CastChange, CastTemplate, Catalog, ChangeOrigin, Persona, PersonaId, Personas,
    PersonasError, PersonasExport, PersonasState, Role, TemplateId,
};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł person: katalog, obsady sesji, zdarzenia `personas.*` na magistrali.
pub struct PersonasModule {
    manifest: ModuleManifest,
    state: Mutex<PersonasState>,
    bus: RwLock<Option<Arc<dyn EventBus>>>,
}

impl PersonasModule {
    /// Moduł z katalogiem wbudowanym.
    pub fn new() -> Result<Self, ManifestError> {
        Self::with_catalog(Catalog::builtin())
    }

    /// Moduł z podanym katalogiem (np. wbudowany + import `.alfa`).
    pub fn with_catalog(catalog: Catalog) -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            state: Mutex::new(PersonasState::new(catalog)),
            bus: RwLock::new(None),
        })
    }

    /// Domyślny szablon obsady nowych sesji (`[personas] default_cast`).
    pub fn set_default_template(&self, id: &TemplateId) -> Result<(), PersonasError> {
        self.state().set_default_template(id)
    }

    /// Własny szablon promptu systemowego (`None` = domyślny); walidowany.
    pub fn set_prompt_template(&self, template: Option<String>) -> Result<(), PersonasError> {
        self.state().set_prompt_template(template)
    }

    /// Import paczki `.alfa` (elementy własne + obsady); publikuje zmiany obsad.
    pub async fn import(&self, export: &PersonasExport) -> Result<Vec<CastChange>, PersonasError> {
        let bus = self.bus()?;
        let changes = self.state().import(export)?;
        publish(&bus, changes.iter().flat_map(CastChange::events)).await;
        Ok(changes)
    }

    fn state(&self) -> MutexGuard<'_, PersonasState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn bus(&self) -> Result<Arc<dyn EventBus>, PersonasError> {
        self.bus
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or(PersonasError::NotStarted)
    }

    async fn change(
        &self,
        f: impl FnOnce(&mut PersonasState) -> Result<CastChange, PersonasError>,
    ) -> Result<CastChange, PersonasError> {
        let bus = self.bus()?;
        let change = f(&mut self.state())?;
        publish(&bus, change.events()).await;
        Ok(change)
    }
}

/// Publikuje zdarzenia; błąd magistrali nie cofa zmiany (stan jest już zapisany).
async fn publish(bus: &Arc<dyn EventBus>, events: impl IntoIterator<Item = Event>) {
    for event in events {
        let _ = bus.publish(event).await;
    }
}

#[async_trait]
impl Personas for PersonasModule {
    fn personas(&self) -> Vec<Persona> {
        self.state().catalog().personas().to_vec()
    }

    fn roles(&self) -> Vec<Role> {
        self.state().catalog().roles().to_vec()
    }

    fn templates(&self) -> Vec<CastTemplate> {
        self.state().catalog().templates().to_vec()
    }

    fn cast(&self, session: &SessionId) -> Cast {
        self.state().cast(session)
    }

    async fn set_cast(
        &self,
        session: &SessionId,
        cast: Cast,
        origin: ChangeOrigin,
    ) -> Result<CastChange, PersonasError> {
        self.change(|s| s.set_cast(session, cast, origin)).await
    }

    async fn set_voice(
        &self,
        session: &SessionId,
        voice: bool,
        origin: ChangeOrigin,
    ) -> Result<CastChange, PersonasError> {
        self.change(|s| s.set_voice(session, voice, origin)).await
    }

    async fn apply_command(
        &self,
        session: &SessionId,
        text: &str,
        origin: ChangeOrigin,
    ) -> Result<Option<CastChange>, PersonasError> {
        let bus = self.bus()?;
        let change = self.state().apply_command(session, text, origin)?;
        if let Some(change) = &change {
            publish(&bus, change.events()).await;
        }
        Ok(change)
    }

    fn resolve_addressee(&self, session: &SessionId, text: &str) -> PersonaId {
        self.state().resolve_addressee(session, text)
    }

    fn system_prompt(
        &self,
        session: &SessionId,
        persona: &PersonaId,
    ) -> Result<String, PersonasError> {
        self.state().system_prompt(session, persona)
    }

    async fn add_persona(&self, persona: Persona) -> Result<(), PersonasError> {
        let bus = self.bus()?;
        let event = self.state().add_persona(persona)?;
        publish(&bus, [event]).await;
        Ok(())
    }

    async fn add_role(&self, role: Role) -> Result<(), PersonasError> {
        self.bus()?;
        self.state().add_role(role)
    }

    async fn add_template(&self, template: CastTemplate) -> Result<(), PersonasError> {
        self.bus()?;
        self.state().add_template(template)
    }

    fn export(&self) -> PersonasExport {
        self.state().export()
    }
}

#[async_trait]
impl Module for PersonasModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut bus = self.bus.write().unwrap_or_else(|p| p.into_inner());
        if bus.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        *bus = Some(ctx.bus);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .map(|_| ())
            .ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        match self.bus.try_read() {
            Ok(guard) if guard.is_some() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::NotStarted,
            Err(_) => HealthStatus::Degraded("stan zajęty".into()),
        }
    }
}
