//! Komendy `agents_*`: obsada ról (natychmiast, do dziennika przez `personas`).

use std::collections::{BTreeMap, BTreeSet};

use personas_contract::{Cast, ChangeOrigin, PersonaId, Personas, RoleId, TemplateId};

use crate::core::AppCore;
use crate::dto::{AgentState, CastTemplateId};
use crate::error::AppError;
use crate::ids;

impl AppCore {
    /// `agents_list`.
    pub async fn agents_list(&self, session_id: String) -> Result<Vec<AgentState>, AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        Ok(self.agents_of(&id))
    }

    /// `agents_set_roles`: role agentki w sesji (zastępują dotychczasowe).
    pub async fn agents_set_roles(
        &self,
        session_id: String,
        agent: String,
        role_ids: Vec<String>,
    ) -> Result<(), AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        let persona = PersonaId::new(agent.as_str());
        if !self
            .inner
            .personas
            .personas()
            .iter()
            .any(|p| p.id == persona)
        {
            return Err(AppError::not_found(format!("Nieznana agentka „{agent}”.")));
        }
        let mut cast = self.inner.personas.cast(&id);
        let roles: BTreeSet<RoleId> = role_ids.iter().map(|r| RoleId::new(r.as_str())).collect();
        cast.assignments.insert(persona, roles);
        cast.template = None;
        self.inner
            .personas
            .set_cast(&id, cast, ChangeOrigin::Ui)
            .await?;
        self.announce_agents(&id);
        Ok(())
    }

    /// `agents_apply_cast`: obsada z szablonu.
    pub async fn agents_apply_cast(
        &self,
        session_id: String,
        template: CastTemplateId,
    ) -> Result<(), AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        let personas = &self.inner.personas;
        let voice = personas.cast(&id).voice;
        let cast = if template == CastTemplateId::Solo {
            let all_roles = personas.roles().into_iter().map(|r| r.id);
            Cast::solo(PersonaId::alfa(), all_roles, voice)
        } else {
            let found = personas
                .templates()
                .into_iter()
                .find(|t| t.id.as_str() == template.as_str())
                .ok_or_else(|| AppError::not_found("Nieznany szablon obsady."))?;
            let mut assignments: BTreeMap<PersonaId, BTreeSet<RoleId>> = personas
                .personas()
                .into_iter()
                .map(|p| (p.id, BTreeSet::new()))
                .collect();
            assignments.extend(found.assignments);
            Cast::new(Some(TemplateId::new(template.as_str())), voice, assignments)
        };
        personas.set_cast(&id, cast, ChangeOrigin::Ui).await?;
        self.announce_agents(&id);
        Ok(())
    }
}
