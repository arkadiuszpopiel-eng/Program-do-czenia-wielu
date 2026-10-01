//! Odpowiedź agentki z narzędziami: zamiast zwykłego strumienia — przebieg `agent-runtime`
//! (rola i prompt z obsady, narzędzia `tools-*` w zakresie katalogu roboczego sesji, decyzje
//! Brokera, dziennik cofania). Zdarzenia `agent.*` → Replay (`AgentStep`, `AgentRunUpdated`),
//! linie kroków w wątku (`ToolCall` z „Cofnij"), karta „czeka na zatwierdzenie", kapsuła
//! aktywności, Oś czasu; odpowiedź końcowa → tura agentki (append-only) z krokami w faktach.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use agent_runtime_contract::{RunEvent, RunOutcome, UsageTotals};
use app_agents::{
    AgentSettings, Projection, RunContext, RunHandle, RunProjector, SpecInput, final_text, keys,
    run_spec,
};
use cost_meter_contract::CostMeter;
use personas_contract::{Persona, PersonaId, Personas, Role};
use providers_contract::Usage;
use sessions_contract::SessionId;

use super::GenRequest;
use super::project::render_closed;
use super::stream::{Chosen, Outcome};
use crate::core::{AppCore, GenHandle};
use crate::dto::{AlfaEvent, TurnError, TurnErrorCode};
use crate::ids;

/// Sterowanie trwającym przebiegiem (steering, anulowanie, stan „czeka").
#[derive(Clone)]
pub(crate) struct RunCtl {
    /// Przebieg.
    pub handle: Arc<RunHandle>,
    /// Agentka.
    pub agent: String,
    /// Czy czeka na zatwierdzenie.
    waiting: Arc<AtomicBool>,
}

impl RunCtl {
    /// Czy agentka czeka na zatwierdzenie.
    pub fn waiting(&self) -> bool {
        self.waiting.load(Ordering::SeqCst)
    }
}

/// Agentka z narzędziami w sesji.
pub(crate) struct AgentSetup {
    workdir: String,
    persona: Persona,
    roles: Vec<Role>,
}

impl AppCore {
    /// Przebieg agentki zamiast czatu, gdy sesja ma katalog roboczy, a role agentki w obsadzie
    /// dają narzędzia (rola bez narzędzi albo sesja bez katalogu — zwykła odpowiedź).
    pub(crate) fn agent_setup(&self, session: &SessionId, agent: &str) -> Option<AgentSetup> {
        let stack = self.inner.agents.as_ref()?;
        let workdir = self.inner.store.workdir(session).ok().flatten()?;
        let id = PersonaId::new(agent);
        let personas = &self.inner.personas;
        let persona = personas.personas().into_iter().find(|p| p.id == id)?;
        let role_ids = personas.cast(session).roles_of(&id);
        let roles: Vec<Role> = personas
            .roles()
            .into_iter()
            .filter(|r| role_ids.contains(&r.id))
            .collect();
        if stack.tools.allowed_for(&roles).is_empty() {
            return None;
        }
        Some(AgentSetup {
            workdir,
            persona,
            roles,
        })
    }

    /// Ustawienia agentek (Ustawienia → Agentki).
    pub(crate) async fn agent_settings(&self) -> AgentSettings {
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
            if let Some(Ok((_, ids::UndoKind::Clipboard, id))) =
                step.undo_token.as_deref().map(ids::parse_any_undo)
            {
                self.rt()
                    .clip_undo
                    .entry(session.clone())
                    .or_default()
                    .insert(id);
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
    core: &AppCore,
    req: &GenRequest,
    handle: &GenHandle,
    setup: AgentSetup,
) -> Outcome {
    let started = Instant::now();
    let Some(stack) = core.inner.agents.clone() else {
        return failed("Narzędzia agentek niepodłączone (Broker albo dziennik cofania).".into());
    };
    let (choice, request) = match super::stream::prepare(core, req).await {
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
        },
        &settings,
        rate,
        window,
    );
    let approval_timeout_ms = spec.approval_timeout_ms;
    let started_run = RunHandle::start(
        choice.provider.clone(),
        &stack.tools,
        Some(core.inner.bus.clone()),
        spec,
    )
    .await;
    let (run, mut feed) = match started_run {
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
    let mut projector = RunProjector::new(ctx, titles, Some(stack.tickets.clone()));
    let mut totals = UsageTotals::default();
    while let Some(env) = feed.next().await {
        if let RunEvent::Usage(u) = &env.event {
            totals = *u;
        }
        let projection = projector.apply(&env);
        core.apply_projection(&req.session, handle, &ctl, &projector, projection);
    }
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
    let fin = final_text(&outcome);
    if !fin.text.is_empty() {
        let blocks = render_closed(&fin.text);
        if let Ok(mut live) = handle.live.lock() {
            live.text.clone_from(&fin.text);
            live.blocks.clone_from(&blocks);
        }
        if let Some(tap) = &req.tap {
            let _ = tap.send(crate::ports::VoiceChunk::Text(fin.text.clone()));
        }
        core.emit(AlfaEvent::TextDelta {
            session_id: req.session.to_string(),
            turn_id,
            text: fin.text.clone(),
            blocks,
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
    }
}
