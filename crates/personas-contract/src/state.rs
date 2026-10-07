//! Stan usługi person (katalog + obsady sesji) — synchroniczny rdzeń dzielony przez `-impl` i `-fake`.
//! Różnią się tylko miejscem, do którego trafiają zdarzenia (magistrala vs nagranie).

use std::collections::BTreeMap;

use core_bus_contract::{Event, SessionId};

use crate::address::resolve_addressee;
use crate::cast::Cast;
use crate::catalog::{Catalog, PersonasExport};
use crate::command::{apply_command, parse_cast_command};
use crate::ids::PersonaId;
use crate::model::{CastTemplate, Persona, Role};
use crate::prompt::{DEFAULT_PROMPT_TEMPLATE, render_system_prompt};
use crate::{CastChange, ChangeOrigin, PersonasError, persona_added_event};

/// Katalog i obsady sesji.
#[derive(Debug, Clone, Default)]
pub struct PersonasState {
    catalog: Catalog,
    casts: BTreeMap<SessionId, Cast>,
    prompt_template: Option<String>,
}

impl PersonasState {
    /// Stan z katalogiem (np. `Catalog::builtin()` albo fixture).
    pub fn new(catalog: Catalog) -> Self {
        Self {
            catalog,
            casts: BTreeMap::new(),
            prompt_template: None,
        }
    }

    /// Katalog.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Domyślny szablon obsady nowych sesji (`[personas] default_cast`).
    pub fn set_default_template(
        &mut self,
        id: &crate::ids::TemplateId,
    ) -> Result<(), PersonasError> {
        Ok(self.catalog.set_default_template(id)?)
    }

    /// Własny szablon promptu (po walidacji) albo `None` = domyślny.
    pub fn set_prompt_template(&mut self, template: Option<String>) -> Result<(), PersonasError> {
        if let Some(t) = &template {
            crate::prompt::validate_template(t)?;
        }
        self.prompt_template = template;
        Ok(())
    }

    /// Obsada sesji (domyślna, gdy nieustawiona).
    pub fn cast(&self, session: &SessionId) -> Cast {
        self.casts
            .get(session)
            .cloned()
            .unwrap_or_else(|| self.catalog.default_cast(false))
    }

    /// Waliduje i zapisuje obsadę; zwraca zmianę (zdarzenia: `CastChange::events`).
    pub fn set_cast(
        &mut self,
        session: &SessionId,
        cast: Cast,
        origin: ChangeOrigin,
    ) -> Result<CastChange, PersonasError> {
        let cast = Cast::new(cast.template, cast.voice, cast.assignments);
        let warnings = self.catalog.validate_cast(&cast)?;
        let before = self.cast(session);
        let diff = cast.diff_from(&before);
        self.casts.insert(session.clone(), cast.clone());
        Ok(CastChange {
            session: session.clone(),
            origin,
            before,
            after: cast,
            diff,
            warnings,
        })
    }

    /// Tryb głosowy/tekstowy sesji (bez Mówczyni w głosowej rolę dostaje Dyrygentka).
    pub fn set_voice(
        &mut self,
        session: &SessionId,
        voice: bool,
        origin: ChangeOrigin,
    ) -> Result<CastChange, PersonasError> {
        let mut cast = self.cast(session);
        cast.voice = voice;
        cast.ensure_speaker();
        self.set_cast(session, cast, origin)
    }

    /// Polecenie obsady z tekstu; `Ok(None)`, gdy to nie polecenie.
    pub fn apply_command(
        &mut self,
        session: &SessionId,
        text: &str,
        origin: ChangeOrigin,
    ) -> Result<Option<CastChange>, PersonasError> {
        let current = self.cast(session);
        let Some(command) = parse_cast_command(text, &self.catalog, &current) else {
            return Ok(None);
        };
        let next = apply_command(&current, &command, &self.catalog)?;
        self.set_cast(session, next, origin).map(Some)
    }

    /// Adresatka: imię wygrywa, inaczej Dyrygentka (awaryjnie pierwsza persona katalogu).
    pub fn resolve_addressee(&self, session: &SessionId, text: &str) -> PersonaId {
        resolve_addressee(text, self.catalog.personas(), &self.cast(session))
            .unwrap_or_else(PersonaId::alfa)
    }

    /// Prompt systemowy persony z rolami w obsadzie sesji.
    pub fn system_prompt(
        &self,
        session: &SessionId,
        persona: &PersonaId,
    ) -> Result<String, PersonasError> {
        let p = self
            .catalog
            .persona(persona)
            .ok_or_else(|| PersonasError::UnknownPersona(persona.clone()))?;
        let role_ids = self.cast(session).roles_of(persona);
        // Kolejność ról jak w katalogu (Dyrygentka przed Koderką), nie alfabetyczna po id.
        let roles: Vec<&Role> = self
            .catalog
            .roles()
            .iter()
            .filter(|r| role_ids.contains(&r.id))
            .collect();
        let template = self
            .prompt_template
            .as_deref()
            .unwrap_or(DEFAULT_PROMPT_TEMPLATE);
        Ok(render_system_prompt(template, p, &roles, &[])?)
    }

    /// Kreator: nowa persona; zwraca zdarzenie do opublikowania.
    pub fn add_persona(&mut self, persona: Persona) -> Result<Event, PersonasError> {
        let event = persona_added_event(&persona);
        self.catalog.add_persona(persona)?;
        Ok(event)
    }

    /// Nowa rola.
    pub fn add_role(&mut self, role: Role) -> Result<(), PersonasError> {
        Ok(self.catalog.add_role(role)?)
    }

    /// Nowy szablon obsady.
    pub fn add_template(&mut self, template: CastTemplate) -> Result<(), PersonasError> {
        Ok(self.catalog.add_template(template)?)
    }

    /// Eksport `.alfa`.
    pub fn export(&self) -> PersonasExport {
        let casts = self
            .casts
            .iter()
            .map(|(s, c)| (s.to_string(), c.clone()))
            .collect();
        self.catalog.export(casts)
    }

    /// Import `.alfa`: elementy własne i obsady (każda walidowana).
    pub fn import(&mut self, export: &PersonasExport) -> Result<Vec<CastChange>, PersonasError> {
        self.catalog.import(export)?;
        let mut changes = Vec::new();
        for (session, cast) in &export.casts {
            let id = SessionId::new(session.as_str());
            changes.push(self.set_cast(&id, cast.clone(), ChangeOrigin::Import)?);
        }
        Ok(changes)
    }
}
