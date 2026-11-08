//! Katalog person, ról i szablonów + walidacja obsad i eksport `.alfa` (wspólne dla `-impl`/`-fake`).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::builtin::{builtin_personas, builtin_roles, builtin_templates};
use crate::cast::{Cast, CastError, CastWarning};
use crate::ids::{PersonaId, RoleId, TemplateId};
use crate::model::{CastTemplate, ModelError, Persona, Role};
use crate::text::fold;

/// Wersja formatu eksportu person w paczce `.alfa`.
pub const EXPORT_VERSION: u32 = 1;

/// Eksport do `.alfa`: elementy własne (wbudowane są w programie) i obsady sesji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PersonasExport {
    /// Wersja formatu ([`EXPORT_VERSION`]).
    pub version: u32,
    /// Domyślny szablon obsady nowych sesji.
    pub default_template: TemplateId,
    /// Własne persony (Kreator).
    pub personas: Vec<Persona>,
    /// Własne role.
    pub roles: Vec<Role>,
    /// Własne szablony obsad.
    pub templates: Vec<CastTemplate>,
    /// Obsady sesji (klucz: identyfikator sesji).
    #[serde(default)]
    pub casts: BTreeMap<String, Cast>,
}

/// Katalog: persony, role, szablony (wbudowane + własne) i domyślny szablon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    personas: Vec<Persona>,
    roles: Vec<Role>,
    templates: Vec<CastTemplate>,
    default_template: TemplateId,
}

impl Default for Catalog {
    fn default() -> Self {
        Self::builtin()
    }
}

impl Catalog {
    /// Katalog wbudowany (4 persony, 9 ról, 4 szablony; domyślny „Standard”).
    pub fn builtin() -> Self {
        Self {
            personas: builtin_personas(),
            roles: builtin_roles(),
            templates: builtin_templates(),
            default_template: TemplateId::standard(),
        }
    }

    /// Wszystkie persony.
    pub fn personas(&self) -> &[Persona] {
        &self.personas
    }

    /// Wszystkie role.
    pub fn roles(&self) -> &[Role] {
        &self.roles
    }

    /// Wszystkie szablony.
    pub fn templates(&self) -> &[CastTemplate] {
        &self.templates
    }

    /// Persona po identyfikatorze.
    pub fn persona(&self, id: &PersonaId) -> Option<&Persona> {
        self.personas.iter().find(|p| &p.id == id)
    }

    /// Rola po identyfikatorze.
    pub fn role(&self, id: &RoleId) -> Option<&Role> {
        self.roles.iter().find(|r| &r.id == id)
    }

    /// Szablon po identyfikatorze.
    pub fn template(&self, id: &TemplateId) -> Option<&CastTemplate> {
        self.templates.iter().find(|t| &t.id == id)
    }

    /// Domyślny szablon obsady nowych sesji.
    pub fn default_template(&self) -> &TemplateId {
        &self.default_template
    }

    /// Ustawia domyślny szablon.
    pub fn set_default_template(&mut self, id: &TemplateId) -> Result<(), CastError> {
        self.template(id)
            .ok_or_else(|| CastError::UnknownTemplate(id.clone()))?;
        self.default_template = id.clone();
        Ok(())
    }

    /// Obsada nowej sesji z domyślnego szablonu.
    pub fn default_cast(&self, voice: bool) -> Cast {
        self.cast_from_template(&self.default_template, None, voice)
            .unwrap_or_else(|_| self.solo_cast(None, voice))
    }

    /// Obsada „Solo” dla agentki (domyślnie pierwszej wbudowanej — Alfy): wszystkie role wbudowane.
    pub fn solo_cast(&self, persona: Option<&PersonaId>, voice: bool) -> Cast {
        let who = persona.cloned().unwrap_or_else(PersonaId::alfa);
        let roles = self
            .roles
            .iter()
            .filter(|r| r.builtin)
            .map(|r| r.id.clone());
        Cast::solo(who, roles, voice)
    }

    /// Obsada z szablonu. W sesji głosowej bez Mówczyni rolę dostaje Dyrygentka.
    pub fn cast_from_template(
        &self,
        id: &TemplateId,
        solo: Option<&PersonaId>,
        voice: bool,
    ) -> Result<Cast, CastError> {
        if *id == TemplateId::solo() {
            let cast = self.solo_cast(solo, voice);
            self.validate_cast(&cast)?;
            return Ok(cast);
        }
        let template = self
            .template(id)
            .ok_or_else(|| CastError::UnknownTemplate(id.clone()))?;
        let mut cast = Cast::new(Some(id.clone()), voice, template.assignments.clone());
        cast.ensure_speaker();
        self.validate_cast(&cast)?;
        Ok(cast)
    }

    /// Walidacja obsady: znane persony i role, dokładnie jedna Dyrygentka, role unikalne
    /// najwyżej raz, w sesji głosowej dokładnie jedna Mówczyni. Zwraca ostrzeżenia
    /// (brak Krytyczki; Krytyczka z rolą autorki — wtedy działa zastępstwo z `Cast::verifier_for`).
    pub fn validate_cast(&self, cast: &Cast) -> Result<Vec<CastWarning>, CastError> {
        if cast.assignments.is_empty() {
            return Err(CastError::Empty);
        }
        for (persona, roles) in &cast.assignments {
            if self.persona(persona).is_none() {
                return Err(CastError::UnknownPersona(persona.clone()));
            }
            if let Some(role) = roles.iter().find(|r| self.role(r).is_none()) {
                return Err(CastError::UnknownRole(role.clone()));
            }
        }
        for role in self.roles.iter().filter(|r| r.unique) {
            let holders = cast.holders(&role.id);
            if holders.len() > 1 {
                return Err(CastError::MultipleHolders {
                    role: role.id.clone(),
                    holders,
                });
            }
        }
        if cast.conductor().is_none() {
            return Err(CastError::NoConductor);
        }
        if cast.voice && cast.speaker().is_none() {
            return Err(CastError::NoSpeakerInVoiceSession);
        }
        let critics = cast.holders(&RoleId::critic());
        if critics.is_empty() {
            return Ok(vec![CastWarning::NoCritic]);
        }
        let warnings = critics
            .into_iter()
            .filter(|critic| {
                cast.assignments.len() > 1
                    && cast
                        .roles_of(critic)
                        .iter()
                        .any(|r| self.role(r).is_some_and(|role| role.author))
            })
            .map(CastWarning::CriticAlsoAuthor)
            .collect();
        Ok(warnings)
    }

    /// Kreator: dodaje własną personę (unikalne id, imię, formy imienia i glif).
    pub fn add_persona(&mut self, persona: Persona) -> Result<(), ModelError> {
        persona.validate()?;
        if persona.builtin {
            return Err(ModelError::Builtin(persona.id.to_string()));
        }
        let forms: Vec<String> = persona.forms.all().iter().map(|(_, f)| fold(f)).collect();
        for other in &self.personas {
            let clash = other.id == persona.id
                || other.glyph == persona.glyph
                || other
                    .forms
                    .all()
                    .iter()
                    .any(|(_, f)| forms.contains(&fold(f)));
            if clash {
                return Err(ModelError::Conflict(format!(
                    "persona `{}` a `{}`",
                    persona.id, other.id
                )));
            }
        }
        self.personas.push(persona);
        Ok(())
    }

    /// Dodaje własną rolę.
    pub fn add_role(&mut self, role: Role) -> Result<(), ModelError> {
        role.validate()?;
        if role.builtin {
            return Err(ModelError::Builtin(role.id.to_string()));
        }
        if self.role(&role.id).is_some() {
            return Err(ModelError::Conflict(format!("rola `{}`", role.id)));
        }
        self.roles.push(role);
        Ok(())
    }

    /// Dodaje własny szablon (przydział musi być poprawną obsadą tekstową).
    pub fn add_template(&mut self, template: CastTemplate) -> Result<(), ModelError> {
        template.validate()?;
        if template.builtin {
            return Err(ModelError::Builtin(template.id.to_string()));
        }
        if self.template(&template.id).is_some() {
            return Err(ModelError::Conflict(format!("szablon `{}`", template.id)));
        }
        let cast = Cast::new(None, false, template.assignments.clone());
        self.validate_cast(&cast)
            .map_err(|e| ModelError::Conflict(e.to_string()))?;
        self.templates.push(template);
        Ok(())
    }

    /// Eksport elementów własnych i podanych obsad.
    pub fn export(&self, casts: BTreeMap<String, Cast>) -> PersonasExport {
        PersonasExport {
            version: EXPORT_VERSION,
            default_template: self.default_template.clone(),
            personas: self
                .personas
                .iter()
                .filter(|p| !p.builtin)
                .cloned()
                .collect(),
            roles: self.roles.iter().filter(|r| !r.builtin).cloned().collect(),
            templates: self
                .templates
                .iter()
                .filter(|t| !t.builtin)
                .cloned()
                .collect(),
            casts,
        }
    }

    /// Import elementów własnych (role → persony → szablony; walidacja jak w Kreatorze).
    pub fn import(&mut self, export: &PersonasExport) -> Result<(), ModelError> {
        if export.version != EXPORT_VERSION {
            return Err(ModelError::InvalidField(format!(
                "version {}",
                export.version
            )));
        }
        for role in &export.roles {
            self.add_role(role.clone())?;
        }
        for persona in &export.personas {
            self.add_persona(persona.clone())?;
        }
        for template in &export.templates {
            self.add_template(template.clone())?;
        }
        self.set_default_template(&export.default_template)
            .map_err(|e| ModelError::Conflict(e.to_string()))
    }
}
