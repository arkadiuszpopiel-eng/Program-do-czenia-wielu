//! Odpowiedź agentki z narzędziami: zamiast zwykłego strumienia — przebieg `agent-runtime`
//! (rola i prompt z obsady, narzędzia `tools-*` w zakresie katalogu roboczego sesji, decyzje
//! Brokera, dziennik cofania). Zdarzenia `agent.*` → Replay (`AgentStep`, `AgentRunUpdated`),
//! linie kroków w wątku (`ToolCall` z „Cofnij"), karta „czeka na zatwierdzenie", kapsuła
//! aktywności, Oś czasu; odpowiedź końcowa → tura agentki (append-only) z krokami w faktach.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use agent_runtime_contract::{RunEvent, RunOutcome, UsageTotals};
use app_agents::{FamilyProjector, RunContext, RunHandle, SpecInput, final_text, run_spec};
use providers_contract::Usage;

use app_api::dto::{AlfaEvent, TurnError, TurnErrorCode};
use app_api::ids::{self};

use crate::engine::{ChatEngine, GenHandle, RunCtl};
use crate::generate::GenRequest;
use crate::outcome::{Chosen, Outcome, provider_error};
use crate::project::render_closed;

use crate::agent_setup::{AgentSetup, failed};

pub(crate) async fn run(
    core: &ChatEngine,
    req: &GenRequest,
    handle: &GenHandle,
    setup: AgentSetup,
) -> Outcome {
    let started = Instant::now();
    let Some(stack) = core.inner.agents.clone() else {
        return failed(
            "Narzędzia agentek niepodłączone (Sędzia Cieni albo dziennik cofania).".into(),
        );
    };
    let (choice, request) = match crate::stream::prepare(core, req, None).await {
        Ok(x) => x,
        Err(e) => return Outcome::failed(e),
    };
    let mut history = request.messages.clone();
    let goal = history.pop().map(|m| m.visible_text()).unwrap_or_default();
    let history = match crate::project_context::history(core, &req.session, &setup.workdir, history)
    {
        Ok(history) => history,
        Err(error) => return Outcome::failed(error),
    };
    let models_before = if crate::conductor_state::billed(core, req) {
        setup
            .recovery
            .as_ref()
            .map(|cp| cp.model_usage.clone())
            .unwrap_or_default()
    } else {
        Default::default()
    };
    let billed_before = if crate::conductor_state::billed(core, req) {
        setup
            .recovery
            .as_ref()
            .map(crate::conductor_state::total)
            .unwrap_or_default()
    } else {
        Default::default()
    };
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
    let mut options = stack.launch.options(&req.session, None);
    options.owner_turn = Some(ids::turn_dto(&req.session, handle.turn));
    core.apply_crew_access(&req.session, &mut options).await;
    let started_run = if let Some(mut cp) = setup.recovery {
        let budget = crate::resume::next_budget(settings.budget(rate), &cp);
        cp.options.crew = options.crew.clone();
        cp.options.owner_turn = options.owner_turn.clone();
        cp.spec.roles = core
            .agent_access(
                &req.session,
                cp.spec.agent.as_str(),
                core.cast_roles(
                    &req.session,
                    &personas_contract::PersonaId::new(cp.spec.agent.as_str()),
                ),
            )
            .await;
        cp.options.continuation = options.continuation.clone();
        cp.options.request_privacy = options.request_privacy;
        RunHandle::resume_saved(
            &stack.launch,
            choice.provider.clone(),
            &stack.tools,
            Some(core.inner.bus.clone()),
            cp,
            budget,
        )
        .await
    } else {
        RunHandle::launch(
            &stack.launch,
            choice.provider.clone(),
            &stack.tools,
            Some(core.inner.bus.clone()),
            spec,
            options,
        )
        .await
    };
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
    // Tury przebiegu głównego wg modelu, który faktycznie odpowiedział: (wejście, wyjście, nano-USD).
    let mut by_model = std::collections::BTreeMap::<String, (u64, u64, Option<u64>)>::new();
    while let Some(env) = family.next().await {
        let (head, projection, main) = family_view.apply(&family, &env);
        match &env.event {
            RunEvent::Usage(u) => totals = *u,
            // Tura bez zużycia (błąd przed treścią, anulowanie) nie robi pustego wpisu kosztu.
            RunEvent::ModelUsage {
                model: Some(model),
                input_tokens,
                output_tokens,
                cost_nano_usd,
            } if input_tokens + output_tokens > 0 || cost_nano_usd.is_some_and(|c| c > 0) => {
                let part = by_model.entry(model.clone()).or_default();
                part.0 += input_tokens;
                part.1 += output_tokens;
                if let Some(c) = cost_nano_usd {
                    part.2 = Some(part.2.unwrap_or(0).saturating_add(*c));
                }
            }
            _ => {}
        }
        if !main {
            let next_agent = if head.finished_at.is_none() {
                &head.agent
            } else {
                &req.agent
            };
            if let Some(r) = core.rt().runs.get_mut(&req.session) {
                next_agent.clone_into(&mut r.agent);
            }
            core.announce_agents(&req.session);
            core.project_task(&req.session, &head, projection);
            continue;
        }
        let steps = family_view.main().tool_steps();
        core.apply_projection_with_tools(
            &req.session,
            handle,
            &ctl,
            family_view.main(),
            projection,
            steps,
        );
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
    if matches!(
        outcome,
        RunOutcome::BudgetExceeded {
            budget: agent_runtime_contract::BudgetKind::Steps
                | agent_runtime_contract::BudgetKind::Wall
                | agent_runtime_contract::BudgetKind::Tokens
                | agent_runtime_contract::BudgetKind::ApprovalWait
        }
    ) && core
        .inner
        .store
        .work_for_turn(&req.session, &ids::turn_dto(&req.session, handle.turn))
        .is_ok_and(|v| !v.is_empty())
    {
        fin.error=Some(TurnError{code:TurnErrorCode::TaskPaused,message:"Osiągnięto miękki limit zadania. Postęp i wyniki operacji są zapisane lokalnie. Zatwierdź kolejną pulę, aby wznowić pracę od zapisanego miejsca.".into(),retry_at:None,provider:None});
    }
    // Błąd dostawcy przerywający przebieg — ten sam komunikat i stan co w zwykłej rozmowie
    // (limit zapytań z „ponów o…”, brak sieci → kolejka offline, odrzucony klucz).
    if let RunOutcome::Failed {
        provider: Some(e),
        model,
        ..
    } = &outcome
    {
        // Model z `Started` przed błędem (Router: `dostawca:model`) — nazwa dostawcy, nie „Router”.
        let target = model.as_deref().and_then(|m| core.inner.brain.target(m));
        let name = target.map_or_else(|| choice.provider_name.clone(), |t| t.provider_name);
        fin.error = Some(provider_error(e, &name));
    }
    // Twarda odmowa kroku kończy przebieg bez odpowiedzi modelu — powód mówi host, nie pusta tura.
    if matches!(outcome, RunOutcome::Refused) && fin.text.is_empty() {
        let blocked = projector
            .tool_steps()
            .last()
            .filter(|s| s.status == app_api::dto::ToolStatus::Error);
        if let Some(step) = blocked {
            fin.text = format!(
                "Zatrzymałam zadanie: krok „{}” został zablokowany (odmowa Sędziego Cieni, Jądra albo brak zgody). Powód jest w krokach zadania; nie próbuję obejść tej blokady.",
                step.label
            );
        }
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
    if let Some(value) = core
        .inner
        .store
        .work(&req.session, run.id().as_str())
        .ok()
        .flatten()
        && let Ok(cp) = serde_json::from_value::<agent_runtime_contract::Checkpoint>(value)
    {
        totals = crate::conductor_state::total(&cp);
        by_model = crate::conductor_state::parts_delta(cp.model_usage, &models_before);
    }
    totals = crate::conductor_state::delta(totals, billed_before);
    let tokens = totals.input_tokens + totals.output_tokens;
    let parts = crate::routing::cost_parts(core.inner.brain.as_ref(), by_model);
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
        parts,
        latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        thinking_ms: None,
        tools: projector.tool_steps().to_vec(),
        approval: projector.approval().cloned(),
        local: None,
    }
}
