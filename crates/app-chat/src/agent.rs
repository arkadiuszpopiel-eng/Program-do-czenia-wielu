//! Odpowiedź agentki z narzędziami: zamiast zwykłego strumienia — przebieg `agent-runtime`
//! (rola i prompt z obsady, narzędzia `tools-*` w zakresie katalogu roboczego sesji, decyzje
//! Brokera, dziennik cofania). Zdarzenia `agent.*` → Replay (`AgentStep`, `AgentRunUpdated`),
//! linie kroków w wątku (`ToolCall` z „Cofnij"), karta „czeka na zatwierdzenie", kapsuła
//! aktywności, Oś czasu; odpowiedź końcowa → tura agentki (append-only) z krokami w faktach.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use agent_runtime_contract::{RunEvent, RunOutcome, UsageTotals};
use app_agents::{
    AgentSettings, FamilyProjector, Projection, RunContext, RunHandle, RunProjector, SpecInput,
    final_text, keys, run_spec,
};
use personas_contract::{Persona, PersonaId, Role};
use providers_contract::Usage;
use sessions_contract::{SessionCatalog, SessionId};

use app_api::dto::{AgentRun, AlfaEvent, TurnError, TurnErrorCode};
use app_api::ids::{self, UndoKind};

use crate::engine::{ChatEngine, GenHandle, RunCtl};
use crate::generate::GenRequest;
use crate::outcome::{Chosen, Outcome, provider_error};
use crate::project::render_closed;

/// Agentka z narzędziami w sesji.
pub(crate) struct AgentSetup {
    workdir: String,
    persona: Persona,
    roles: Vec<Role>,
}

impl ChatEngine {
    /// Role agentki w obsadzie sesji.
    fn cast_roles(&self, session: &SessionId, who: &PersonaId) -> Vec<Role> {
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
    pub(crate) fn agent_setup(&self, session: &SessionId, agent: &str) -> Option<AgentSetup> {
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
        if stack.tools.allowed_for(&roles).is_empty() {
            return None;
        }
        Some(AgentSetup {
            workdir,
            persona,
            roles,
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
    fn session_workdir(&self, session: &SessionId) -> Option<String> {
        match self.inner.store.workdir_choice(session).ok()? {
            Some(choice) => choice,
            None => {
                let dir = self.inner.sessions.session(session).ok()?.workdir;
                std::fs::create_dir_all(&dir).ok()?;
                Some(dir.to_string_lossy().into_owned())
            }
        }
    }

    /// Ustawienia agentek (Ustawienia → Agentki).
    pub async fn agent_settings(&self) -> AgentSettings {
        let d = AgentSettings::default();
        let num = |v: Option<serde_json::Value>, default: u64| {
            v.and_then(|v| v.as_f64())
                .filter(|n| n.is_finite() && *n >= 0.0)
                .map_or(default, |n| n.round() as u64)
        };
        let u32_of = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        AgentSettings {
            max_steps: u32_of(num(
                self.config_value(keys::MAX_STEPS).await,
                d.max_steps.into(),
            )),
            max_minutes: u32_of(num(
                self.config_value(keys::MAX_MINUTES).await,
                d.max_minutes.into(),
            )),
            max_cost_grosze: num(self.config_value(keys::MAX_COST_PLN).await, 0) * 100,
            approval_timeout_s: match self.inner.approval_timeout {
                Some(limit) => u32_of(limit.as_secs().max(1)),
                None => u32_of(num(
                    self.config_value(keys::APPROVAL_TIMEOUT_S).await,
                    d.approval_timeout_s.into(),
                )),
            },
            verify: self.config_bool(keys::VERIFY, d.verify).await,
        }
    }

    /// Stosuje projekcję: zdarzenia UI, tura na żywo, zapis Replay i Osi czasu, stan agentki.
    fn apply_projection(
        &self,
        session: &SessionId,
        handle: &GenHandle,
        ctl: &RunCtl,
        projector: &RunProjector,
        p: Projection,
    ) {
        if let Ok(mut live) = handle.live.lock() {
            live.tools = projector.tool_steps().to_vec();
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

    /// Projekcja przebiegu zadania (wykonawczyni zadań): kroki i nagłówek do Replay, zdarzenia
    /// UI, Oś czasu.
    pub fn project_task(&self, session: &SessionId, run: &AgentRun, p: Projection) {
        let store = &self.inner.store;
        for step in &p.steps {
            if let Err(e) = store.push_step(session, &run.id, step) {
                tracing::warn!(error = %e, "zapis kroku zadania nie powiódł się");
            }
        }
        if p.run_changed
            && let Err(e) = store.push_run(session, run)
        {
            tracing::warn!(error = %e, "zapis przebiegu zadania nie powiódł się");
        }
        self.inner.events.emit_all(p.events);
        for event in p.timeline {
            if let Err(e) = store.push_timeline(session, &event) {
                tracing::warn!(error = %e, "zapis osi czasu nie powiódł się");
            }
            self.emit(AlfaEvent::TimelineAppended { event });
        }
    }
}

fn failed(message: String) -> Outcome {
    Outcome::failed(TurnError {
        code: TurnErrorCode::Provider,
        message,
        retry_at: None,
        provider: None,
    })
}

/// Przebieg agentki jako odpowiedź na turę.
pub(crate) async fn run(
    core: &ChatEngine,
    req: &GenRequest,
    handle: &GenHandle,
    setup: AgentSetup,
) -> Outcome {
    let started = Instant::now();
    let Some(stack) = core.inner.agents.clone() else {
        return failed("Narzędzia agentek niepodłączone (Broker albo dziennik cofania).".into());
    };
    let (choice, request) = match crate::stream::prepare(core, req, None).await {
        Ok(x) => x,
        Err(e) => return Outcome::failed(e),
    };
    let mut history = request.messages.clone();
    let goal = history.pop().map(|m| m.visible_text()).unwrap_or_default();
    let settings = core.agent_settings().await;
    let rate = u64::from(core.inner.costs.current_rate().rate_e4);
    let window = core.inner.broker.approval_window();
    let role = setup.roles.first().map(|r| r.id.as_str().to_owned());
    let spec = run_spec(
        SpecInput {
            session: req.session.clone(),
            persona: setup.persona,
            roles: setup.roles,
            goal: goal.clone(),
            origin: req.origin,
            model: choice.model.clone(),
            tools: stack.tools.names(),
            workdir: setup.workdir.clone(),
            history,
            environment: stack.tools.environment().await,
        },
        &settings,
        rate,
        window,
    );
    let approval_timeout_ms = spec.approval_timeout_ms;
    // v1: obsada sesji → delegacja (`delegate_task`) i Krytyczka jako podprzebiegi w Replay.
    let options = stack.launch.options(&req.session, None);
    let started_run = RunHandle::launch(
        &stack.launch,
        choice.provider.clone(),
        &stack.tools,
        Some(core.inner.bus.clone()),
        spec,
        options,
    )
    .await;
    let (run, mut family) = match started_run {
        Ok(x) => x,
        Err(e) => return failed(format!("Nie udało się uruchomić zadania agentki: {e}")),
    };
    let run = Arc::new(run);
    let ctl = RunCtl {
        handle: run.clone(),
        agent: req.agent.clone(),
        waiting: Arc::default(),
    };
    core.rt().runs.insert(req.session.clone(), ctl.clone());
    core.announce_agents(&req.session);
    let cancel = handle.cancel.clone();
    let linked = run.clone();
    let linker = tokio::spawn(async move {
        cancel.cancelled().await;
        linked.cancel();
    });
    let titles: BTreeMap<String, String> = stack
        .tools
        .all()
        .iter()
        .map(|t| (t.manifest().name.clone(), t.manifest().title.clone()))
        .collect();
    let turn_id = ids::turn_dto(&req.session, handle.turn);
    let ctx = RunContext {
        session: req.session.clone(),
        turn_id: Some(turn_id.clone()),
        agent: req.agent.clone(),
        role,
        run: run.id().as_str().to_owned(),
        goal,
        workdir: Some(setup.workdir),
        budget: settings.view(),
        broker_window: window,
        approval_timeout_ms,
        usd_pln_e4: rate,
        started_at: chrono::Utc::now(),
        task_id: None,
    };
    let mut family_view = FamilyProjector::new(ctx, titles, Some(stack.tickets.clone()));
    let mut totals = UsageTotals::default();
    while let Some(env) = family.next().await {
        let (head, projection, main) = family_view.apply(&family, &env);
        if !main {
            core.project_task(&req.session, &head, projection);
            continue;
        }
        if let RunEvent::Usage(u) = &env.event {
            totals = *u;
        }
        core.apply_projection(&req.session, handle, &ctl, family_view.main(), projection);
    }
    let projector = family_view.main();
    linker.abort();
    {
        let mut rt = core.rt();
        if rt
            .runs
            .get(&req.session)
            .is_some_and(|r| Arc::ptr_eq(&r.handle, &run))
        {
            rt.runs.remove(&req.session);
        }
    }
    let outcome = projector
        .outcome()
        .cloned()
        .unwrap_or(RunOutcome::Cancelled);
    let mut fin = final_text(&outcome);
    // Błąd dostawcy przerywający przebieg — ten sam komunikat i stan co w zwykłej rozmowie
    // (limit zapytań z „ponów o…”, brak sieci → kolejka offline, odrzucony klucz).
    if let RunOutcome::Failed {
        provider: Some(e), ..
    } = &outcome
    {
        fin.error = Some(provider_error(e, &choice.provider_name));
    }
    if !fin.text.is_empty() {
        let blocks = render_closed(&fin.text);
        if let Ok(mut live) = handle.live.lock() {
            live.text.clone_from(&fin.text);
            live.blocks.clone_from(&blocks);
        }
        if let Some(tap) = &req.tap {
            let _ = tap.send(app_api::ports::VoiceChunk::Text(fin.text.clone()));
        }
        core.emit(AlfaEvent::TextDelta {
            session_id: req.session.to_string(),
            turn_id,
            text: fin.text.clone(),
            blocks,
            offset: 0,
        });
    }
    let tokens = totals.input_tokens + totals.output_tokens;
    Outcome {
        text: fin.text,
        thinking: Vec::new(),
        status: fin.status,
        stop: fin.stop,
        error: fin.error,
        usage: (tokens > 0).then(|| Usage {
            input_tokens: totals.input_tokens,
            output_tokens: totals.output_tokens,
            ..Usage::default()
        }),
        cost_nano_usd: (totals.cost_nano_usd > 0).then_some(totals.cost_nano_usd),
        chosen: Some(Chosen {
            provider_id: choice.provider_id.clone(),
            provider_name: choice.provider_name.clone(),
            account: choice.account.clone(),
            model: choice.model.clone(),
        }),
        latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        thinking_ms: None,
        tools: projector.tool_steps().to_vec(),
        approval: projector.approval().cloned(),
        local: None,
    }
}
