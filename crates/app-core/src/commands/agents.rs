//! Komendy `agents_*`: obsada ról (natychmiast, do dziennika przez `personas`), przebiegi agentek
//! z narzędziami (Replay), wiadomość w trakcie zadania, „uruchom w terminalu".

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use personas_contract::{Cast, ChangeOrigin, PersonaId, Personas, RoleId, TemplateId};

use crate::core::AppCore;
use crate::dto::{
    AgentRunDetail, AgentState, CastTemplateId, EventLevel, IntentKind, TimelineKind,
};
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

impl AppCore {
    /// `agents_runs`: przebiegi agentek w sesji z krokami (Replay na Osi czasu).
    pub async fn agents_runs(&self, session_id: String) -> Result<Vec<AgentRunDetail>, AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        self.inner.store.runs(&id)
    }

    /// `agents_steer`: wiadomość właściciela w trakcie zadania — agentka uwzględnia ją w następnym
    /// kroku (PLAN §9.6); bez trwającego zadania — błąd (wiadomość wysyła się zwykle).
    pub async fn agents_steer(&self, session_id: String, text: String) -> Result<(), AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        let text = text.trim().to_owned();
        if text.is_empty() || text.chars().count() > MAX_STEER_CHARS {
            return Err(AppError::invalid(format!(
                "Wiadomość w trakcie zadania musi mieć od 1 do {MAX_STEER_CHARS} znaków."
            )));
        }
        let run = self
            .rt()
            .runs
            .get(&id)
            .map(|r| r.handle.clone())
            .ok_or_else(|| AppError::invalid("Agentka nie wykonuje teraz zadania w tej sesji."))?;
        run.steer(text)
            .map_err(|e| AppError::invalid(format!("Zadanie już się zakończyło: {e}")))
    }

    /// `agents_open_terminal` ⟶ terminal w katalogu kroku „uruchom w terminalu" — polecenie nie
    /// jest wykonywane (właściciel kopiuje je i uruchamia sam).
    pub async fn agents_open_terminal(&self, step_id: String) -> Result<(), AppError> {
        let (session, run, _) = ids::parse_step(&step_id)?;
        self.ensure_session(&session)?;
        let run_id = ids::run_dto(&session, &run);
        let intent = self
            .inner
            .store
            .runs(&session)?
            .into_iter()
            .filter(|r| r.run.id == run_id)
            .flat_map(|r| r.steps)
            .find(|s| s.id == step_id)
            .and_then(|s| s.intent)
            .filter(|i| i.kind == IntentKind::OpenInTerminal)
            .ok_or_else(|| {
                AppError::not_found("Ten krok nie proponuje uruchomienia w terminalu.")
            })?;
        let cwd = intent
            .cwd
            .clone()
            .ok_or_else(|| AppError::invalid("Brak katalogu polecenia."))?;
        let shell_kind = intent.shell.clone().unwrap_or_else(|| "pwsh".into());
        let shell = self.inner.shell.clone();
        let dir = cwd.clone();
        tokio::task::spawn_blocking(move || shell.open_terminal(Path::new(&dir), &shell_kind))
            .await
            .map_err(|e| AppError::internal(format!("terminal: {e}")))??;
        self.timeline_note(
            &session,
            TimelineKind::Ui,
            EventLevel::Info,
            format!("Otwarto terminal w „{cwd}” (polecenie do skopiowania)"),
            intent.command,
        );
        Ok(())
    }
}

/// Najdłuższa wiadomość w trakcie zadania.
const MAX_STEER_CHARS: usize = 4000;
