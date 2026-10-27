//! Uruchomienie pojedynczego podzadania zleconego przez Dyrygentkę (`conductor`) wybranej agentce
//! z obsady (`Delta`, `Gama`, `Beta`) z dynamicznym przełączeniem aktywnej agentki w UI
//! i projekcją kroków narzędzi na żywo do bieżącej tury.

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_runtime_contract::{RunEvent, RunOutcome, UsageTotals};
use app_agents::{FamilyProjector, RunContext, RunHandle, SpecInput, run_spec};
use app_api::dto::ToolStep;
use chrono::Utc;
use personas_contract::{Persona, Role, RoleId};
use providers_contract::Message;

use crate::engine::{ChatEngine, GenHandle, RunCtl};
use crate::generate::GenRequest;

pub(crate) use crate::child_resolve::resolve_delegation;

/// Kontekst uruchomienia pojedynczego podzadania agentki.
#[derive(Clone)]
pub(crate) struct ChildExec<'a> {
    pub input: serde_json::Value,
    pub call_id: String,
    pub budget: &'a crate::conductor_budget::ConductorBudget,
    pub core: &'a ChatEngine,
    pub req: &'a GenRequest,
    pub handle: &'a GenHandle,
    pub parent_run_id: &'a str,
    pub tid: &'a str,
    pub workdir: &'a str,
    pub history: Vec<Message>,
}

/// Uruchamia podzadanie wybranej agentki (`Delta`, `Gama`, `Beta`), przełączając aktywną agentkę
/// w UI i dopisując jej kroki narzędzi na żywo do bieżącej tury.
pub(crate) async fn execute_child(
    cx: &ChildExec<'_>,
    persona: Persona,
    roles: Vec<Role>,
    goal: String,
    all_tools: &mut Vec<ToolStep>,
    by_model: &mut BTreeMap<String, (u64, u64, Option<u64>)>,
    model_override: Option<String>,
) -> (Result<String, String>, UsageTotals) {
    let Some(stack) = cx.core.inner.agents.clone() else {
        return (
            Err("Narzędzia agentek niepodłączone.".into()),
            UsageTotals::default(),
        );
    };
    let settings = cx.core.agent_settings().await;
    let roles = cx
        .core
        .agent_access(&cx.req.session, persona.id.as_str(), roles)
        .await;
    let recovery = crate::child_state::recover(cx, &goal, persona.id.as_str());
    let prior = crate::child_state::accounted(cx);
    let (retry_roles, retry_goal, retry_model) =
        (roles.clone(), goal.clone(), model_override.clone());
    if let Some(cp) = recovery.as_ref()
        && let Some(RunOutcome::Completed { .. }) = &cp.finished
    {
        return crate::child_state::completed(cx, cp, prior, &persona.name, all_tools, by_model);
    }
    let rate = u64::from(cx.core.inner.costs.current_rate().rate_e4);
    let window = cx.core.inner.broker.approval_window();
    let role_id = roles.first().map(|r| r.id.as_str().to_owned());
    let role_name = roles
        .first()
        .map_or_else(|| persona.name.clone(), |r| r.name.clone());
    let model = model_override
        .or_else(|| {
            let r = role_id.as_deref()?;
            stack
                .launch
                .options(&cx.req.session, None)
                .crew?
                .models
                .get(&RoleId::new(r))
                .cloned()
        })
        .unwrap_or_else(|| router_contract::AUTO_MODEL.to_owned());
    let (history, handoff, owner_context) = match crate::conductor_handoff::prepare(cx) {
        Ok(data) => data,
        Err(error) => {
            cx.budget.fail_storage(&error);
            return (Err(error), UsageTotals::default());
        }
    };
    let mut spec = run_spec(
        SpecInput {
            session: cx.req.session.clone(),
            persona: persona.clone(),
            roles,
            goal: goal.clone(),
            origin: cx.req.origin,
            model,
            tools: stack.tools.names(),
            workdir: cx.workdir.to_owned(),
            history,
            environment: stack.tools.environment().await,
        },
        &settings,
        rate,
        window,
    );
    let choice = match crate::stage_route::choose(cx.core, cx.req, &spec).await {
        Ok(choice) => choice,
        Err(e) => return (Err(e), UsageTotals::default()),
    };
    spec.model.clone_from(&choice.model);
    let approval_timeout_ms = spec.approval_timeout_ms;
    let mut options = stack.launch.options(
        &cx.req.session,
        Some(format!("delegacja: {} ({})", persona.name, role_name)),
    );
    crate::conductor_handoff::taint(&mut options, &handoff, &owner_context);
    options.depth = 1;
    options.owner_turn = Some(cx.tid.to_owned());
    options.owner_call = Some(cx.call_id.clone());
    cx.core
        .apply_crew_access(&cx.req.session, &mut options)
        .await;
    let Some(remaining) = cx.budget.remaining() else {
        cx.budget.pause();
        return (
            Err(
                "Wspólny limit kroków, czasu, tokenów albo kosztu zadania został osiągnięty."
                    .into(),
            ),
            UsageTotals::default(),
        );
    };
    spec.budget = remaining;
    let started_run = if let Some(mut cp) = recovery {
        let mut budget = crate::resume::next_budget(remaining, &cp);
        budget.max_cost_micro_usd = remaining.max_cost_micro_usd.map(|n| {
            n.saturating_add(
                crate::conductor_state::total(&cp)
                    .cost_nano_usd
                    .div_ceil(1000),
            )
        });
        crate::conductor_handoff::refresh_recovery(&mut cp, &options);
        cp.options.crew = options.crew.clone();
        cp.options.owner_turn = options.owner_turn.clone();
        cp.spec.roles = spec.roles.clone();
        cp.options.continuation = options.continuation.clone();
        cp.options.request_privacy = options.request_privacy;
        RunHandle::resume_saved(
            &stack.launch,
            choice.provider.clone(),
            &stack.tools,
            Some(cx.core.inner.bus.clone()),
            cp,
            budget,
        )
        .await
    } else {
        RunHandle::launch(
            &stack.launch,
            choice.provider.clone(),
            &stack.tools,
            Some(cx.core.inner.bus.clone()),
            spec,
            options,
        )
        .await
    };
    let (run, mut family) = match started_run {
        Ok(x) => x,
        Err(e) => {
            return (
                Err(format!(
                    "Nie udało się uruchomić agentki {}: {e}",
                    persona.name
                )),
                UsageTotals::default(),
            );
        }
    };
    let run = Arc::new(run);
    if let Err(error) = crate::child_state::bind(cx, run.id().as_str()) {
        run.cancel();
        cx.budget.fail_storage(&error);
        return (
            Err(format!("Nie zapisano powiązania zadania: {error}")),
            UsageTotals::default(),
        );
    }
    let ctl = RunCtl {
        handle: run.clone(),
        agent: persona.id.as_str().to_owned(),
        waiting: Arc::default(),
    };
    cx.core
        .rt()
        .runs
        .insert(cx.req.session.clone(), ctl.clone());
    cx.core.announce_agents(&cx.req.session);
    let cancel = cx.handle.cancel.clone();
    let linked = run.clone();
    let linker = tokio::spawn(async move {
        cancel.cancelled().await;
        linked.cancel();
    });
    let titles: BTreeMap<String, String> = stack
        .tools
        .all()
        .iter()
        .map(|t| {
            (
                t.manifest().name.clone(),
                format!("{} · {}", persona.name, t.manifest().title),
            )
        })
        .collect();
    let ctx = RunContext {
        session: cx.req.session.clone(),
        turn_id: Some(cx.tid.to_owned()),
        agent: persona.id.as_str().to_owned(),
        role: role_id,
        run: run.id().as_str().to_owned(),
        goal,
        workdir: Some(cx.workdir.to_owned()),
        budget: settings.view(),
        broker_window: window,
        approval_timeout_ms,
        usd_pln_e4: rate,
        started_at: Utc::now(),
        task_id: None,
    };
    let mut family_view = FamilyProjector::new(ctx, titles, Some(stack.tickets.clone()))
        .with_main_parent(
            cx.parent_run_id.to_owned(),
            Some(format!("delegacja → {} ({})", persona.name, role_name)),
        );
    let base_models = by_model.clone();
    let mut usage_by_run = BTreeMap::new();
    let base_tools_len = all_tools.len();
    while let Some(env) = family.next().await {
        cx.budget.observe(&env.event);
        let (head, projection, main) = family_view.apply(&family, &env);
        match &env.event {
            RunEvent::Usage(u) => {
                usage_by_run.insert(env.run.clone(), *u);
            }
            RunEvent::ModelUsage {
                model: Some(m),
                input_tokens,
                output_tokens,
                cost_nano_usd,
            } if input_tokens + output_tokens > 0 || cost_nano_usd.is_some_and(|c| c > 0) => {
                let part = by_model.entry(m.clone()).or_default();
                part.0 += input_tokens;
                part.1 += output_tokens;
                if let Some(c) = cost_nano_usd {
                    part.2 = Some(part.2.unwrap_or(0).saturating_add(*c));
                }
            }
            _ => {}
        }
        if !main {
            cx.core.project_task(&cx.req.session, &head, projection);
            continue;
        }
        all_tools.truncate(base_tools_len);
        all_tools.extend_from_slice(family_view.main().tool_steps());
        cx.core.apply_projection_with_tools(
            &cx.req.session,
            cx.handle,
            &ctl,
            family_view.main(),
            projection,
            all_tools,
        );
    }
    linker.abort();
    {
        let mut rt = cx.core.rt();
        if rt
            .runs
            .get(&cx.req.session)
            .is_some_and(|r| Arc::ptr_eq(&r.handle, &run))
        {
            rt.runs.remove(&cx.req.session);
        }
    }
    cx.core.announce_agents(&cx.req.session);
    let outcome = family_view
        .main()
        .outcome()
        .cloned()
        .unwrap_or(RunOutcome::Cancelled);
    let res = crate::child_state::result(&outcome, &persona.name, &role_name);
    let mut totals = crate::child_state::sum_usage(usage_by_run.into_values());
    if let Some(value) = cx
        .core
        .inner
        .store
        .work(&cx.req.session, run.id().as_str())
        .ok()
        .flatten()
        && let Ok(cp) = serde_json::from_value::<agent_runtime_contract::Checkpoint>(value)
    {
        *by_model = crate::child_state::models(cx, &cp, &base_models);
        totals = crate::conductor_state::delta(crate::conductor_state::total(&cp), prior);
        cx.budget.charge(totals);
        if let Err(error) = crate::child_state::progress(cx, &cp, totals, all_tools, by_model) {
            cx.budget.fail_storage(&error);
            return (Err(format!("Nie zapisano postępu: {error}")), totals);
        }
        if let RunOutcome::BudgetExceeded { budget } = &outcome {
            if cx.budget.auto_for(*budget) && cx.budget.remaining().is_some() {
                let mut next = cx.clone();
                next.input["__run"] = cp.run.as_str().into();
                next.input["__accounted"] =
                    serde_json::to_value(crate::conductor_state::total(&cp)).unwrap_or_default();
                next.input["__accounted_models"] =
                    serde_json::to_value(&cp.model_usage).unwrap_or_default();
                let (result, usage) = Box::pin(execute_child(
                    &next,
                    persona,
                    retry_roles,
                    retry_goal,
                    all_tools,
                    by_model,
                    retry_model,
                ))
                .await;
                crate::conductor_state::add(&mut totals, usage);
                return (result, totals);
            }
            cx.budget.pause();
        }
    }
    (res, totals)
}

pub(crate) use crate::conductor_record::record_delegation_step;
