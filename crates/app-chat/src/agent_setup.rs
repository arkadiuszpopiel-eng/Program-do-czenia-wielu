//! Konfiguracja agentki i projekcja aktywnego przebiegu.
use std::sync::atomic::Ordering;

use app_agents::{Projection, RunProjector};
use personas_contract::{Persona, PersonaId, Role};
use sessions_contract::{SessionCatalog, SessionId};

use app_api::dto::{AlfaEvent, TurnError, TurnErrorCode};
use app_api::ids::{self, UndoKind};

use crate::engine::{ChatEngine, GenHandle, RunCtl};
use crate::generate::GenRequest;
use crate::outcome::Outcome;

/// Agentka z narzędziami w sesji.
pub(crate) struct AgentSetup {
    pub(crate) workdir: String,
    pub(crate) persona: Persona,
    pub(crate) roles: Vec<Role>,
    pub(crate) recovery: Option<agent_runtime_contract::Checkpoint>,
}

impl ChatEngine {
    pub(crate) fn recovery_setup(&self, req: &GenRequest) -> Option<AgentSetup> {
        let cp = crate::resume::saved_native(self, req)?;
        Some(AgentSetup {
            workdir: cp.spec.workdir.clone()?,
            persona: cp.spec.persona.clone(),
            roles: cp.spec.roles.clone(),
            recovery: Some(cp),
        })
    }
    /// Role agentki w obsadzie sesji.
    pub(crate) fn cast_roles(&self, session: &SessionId, who: &PersonaId) -> Vec<Role> {
        let ids = self.inner.personas.cast(session).roles_of(who);
        self.inner
            .personas
            .roles()
            .into_iter()
            .filter(|r| ids.contains(&r.id))
            .collect()
    }

    /// Przebieg agentki zamiast czatu, gdy sesja ma katalog roboczy, a role agentki w obsadzie
    /// dają narzędzia (rola bez narzędzi albo sesja z jawnie wyłączonym katalogiem — zwykła
    /// odpowiedź).
    pub(crate) async fn agent_setup(&self, session: &SessionId, agent: &str) -> Option<AgentSetup> {
        let stack = self.inner.agents.as_ref()?;
        let workdir = self.session_workdir(session)?;
        let id = PersonaId::new(agent);
        let persona = self
            .inner
            .personas
            .personas()
            .into_iter()
            .find(|p| p.id == id)?;
        let roles = self.cast_roles(session, &id);
        if self.orchestrates(agent, &roles).await {
            return None;
        }
        let roles = self.agent_access(session, agent, roles).await;
        let delegates = roles
            .iter()
            .any(|r| !r.read_only && r.tools.iter().any(|g| g == "delegate"));
        if stack.tools.allowed_for(&roles).is_empty() && !delegates {
            return None;
        }
        Some(AgentSetup {
            workdir,
            persona,
            roles,
            recovery: None,
        })
    }

    /// Imiona innych agentek obsady, które działają na komputerze (role z poleceniami albo
    /// sterowaniem GUI, np. Wykonawczyni) — dla agentki bez narzędzi (Dyrygentka): wie, komu
    /// przekazać pracę, zamiast odpowiadać, że dostępu nie ma. Pusta lista, gdy narzędzia są
    /// niepodłączone albo katalog wyłączony.
    pub(crate) fn crew_with_tools(&self, session: &SessionId, agent: &str) -> Vec<String> {
        let Some(stack) = self.inner.agents.as_ref() else {
            return Vec::new();
        };
        if self.session_workdir(session).is_none() {
            return Vec::new();
        }
        let id = PersonaId::new(agent);
        let personas = self.inner.personas.personas();
        self.inner
            .personas
            .cast(session)
            .personas()
            .into_iter()
            .filter(|p| *p != id)
            .filter(|p| {
                let roles = self.cast_roles(session, p);
                let computer = roles
                    .iter()
                    .any(|r| r.tools.iter().any(|g| g == "shell" || g == "gui.control"));
                computer && !stack.tools.allowed_for(&roles).is_empty()
            })
            .filter_map(|p| personas.iter().find(|x| x.id == p).map(|x| x.name.clone()))
            .collect()
    }

    /// Katalog roboczy sesji; gdy nigdy go nie wybrano — katalog sesji (jak „Domyślny” w oknie
    /// rozmowy), żeby agentki nie traciły narzędzi po cichu. Jawne „bez katalogu” — `None`.
    pub fn session_workdir(&self, session: &SessionId) -> Option<String> {
        match self.inner.store.workdir_choice(session).ok()? {
            Some(choice) => choice,
            None => {
                let dir = self.inner.sessions.session(session).ok()?.workdir;
                std::fs::create_dir_all(&dir).ok()?;
                Some(dir.to_string_lossy().into_owned())
            }
        }
    }

    /// Stosuje projekcję: zdarzenia UI, tura na żywo, zapis Replay i Osi czasu, stan agentki.
    pub(crate) fn apply_projection_with_tools(
        &self,
        session: &SessionId,
        handle: &GenHandle,
        ctl: &RunCtl,
        projector: &RunProjector,
        p: Projection,
        tools: &[app_api::dto::ToolStep],
    ) {
        if let Ok(mut live) = handle.live.lock() {
            live.tools = tools.to_vec();
            live.approval = projector.approval().cloned();
        }
        let store = &self.inner.store;
        let run_id = projector.run().id.clone();
        for step in &p.steps {
            if let Err(e) = store.push_step(session, &run_id, step) {
                tracing::warn!(error = %e, "zapis kroku przebiegu nie powiódł się");
            }
            // Kroki cofane poza dziennikiem (schowek, zmienne) należą do sesji przebiegu.
            if let Some(Ok((_, kind @ (UndoKind::Clipboard | UndoKind::System), id))) =
                step.undo_token.as_deref().map(ids::parse_any_undo)
            {
                self.rt()
                    .owned_undo
                    .entry(session.clone())
                    .or_default()
                    .insert((kind, id));
            }
        }
        if p.run_changed
            && let Err(e) = store.push_run(session, projector.run())
        {
            tracing::warn!(error = %e, "zapis przebiegu nie powiódł się");
        }
        self.inner.events.emit_all(p.events);
        for event in p.timeline {
            if let Err(e) = store.push_timeline(session, &event) {
                tracing::warn!(error = %e, "zapis osi czasu nie powiódł się");
            }
            self.emit(AlfaEvent::TimelineAppended { event });
        }
        let waiting = projector.waiting();
        if ctl.waiting.swap(waiting, Ordering::SeqCst) != waiting {
            self.announce_agents(session);
        }
    }
}

pub(crate) fn failed(message: String) -> Outcome {
    Outcome::failed(TurnError {
        code: TurnErrorCode::Provider,
        message,
        retry_at: None,
        provider: None,
    })
}
