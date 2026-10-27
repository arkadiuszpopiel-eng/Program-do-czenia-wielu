//! Zakończenie i przyrostowe rozliczenie zapisanej orkiestracji.
use super::conductor_budget::ConductorBudget;
use super::conductor_state::Saved;
use super::outcome::{Chosen, Outcome, classify, millis};
use super::{ChatEngine, GenHandle, GenRequest};
use agent_runtime_contract::UsageTotals;
use app_agents::short;
use app_api::{
    dto::{AlfaEvent, RunState, iso},
    ids,
};
use chrono::Utc;
use lib_markdown::IncrementalRenderer;
use providers_contract::Usage;
use std::{collections::BTreeMap, time::Instant};
pub(crate) struct Completion<'a> {
    pub core: &'a ChatEngine,
    pub req: &'a GenRequest,
    pub handle: &'a GenHandle,
    pub work: Saved,
    pub budget: &'a ConductorBudget,
    pub renderer: IncrementalRenderer,
    pub sent_units: u64,
    pub started: Instant,
    pub chosen: Chosen,
    pub routed: bool,
    pub cancelled: bool,
    pub exhausted: bool,
    pub billed_before: UsageTotals,
    pub parts_before: BTreeMap<String, (u64, u64, Option<u64>)>,
}
pub(crate) fn finish(cx: Completion<'_>) -> Outcome {
    let Completion {
        core,
        req,
        handle,
        mut work,
        budget,
        renderer,
        sent_units,
        started,
        mut chosen,
        routed,
        cancelled,
        exhausted,
        billed_before,
        parts_before,
    } = cx;
    let sid = req.session.to_string();
    let tid = ids::turn_dto(&req.session, handle.turn);
    let text = crate::conductor_summary::finish(
        core,
        handle,
        sid.clone(),
        tid.clone(),
        renderer,
        sent_units,
        &work.current,
        &work.summaries,
        &work.evidence,
    );
    if let Some(m) = work.current.model.clone().filter(|_| !routed) {
        chosen.model = m;
    }
    let (mut status, stop, mut error) = classify(cancelled, &work.current, &chosen.provider_name);
    if exhausted {
        status = app_api::dto::TurnStatus::Error;
        error = Some(app_api::dto::TurnError { code: app_api::dto::TurnErrorCode::TaskPaused, message: "Osiągnięto limit zadania. Stan Dyrygentki, etapy i wyniki są zapisane lokalnie; zatwierdź kolejną pulę pracy, aby wznowić.".into(), retry_at: None, provider: None });
    }
    if let Some(storage) = budget.storage_error() {
        error = Some(app_api::dto::TurnError {
            code: app_api::dto::TurnErrorCode::Provider,
            message: format!(
                "Zatrzymano pracę: nie udało się zapisać najnowszego postępu lokalnie ({storage}). Sprawdź wolne miejsce; stan ostatniej rozpoczętej operacji może wymagać sprawdzenia."
            ),
            retry_at: None,
            provider: None,
        });
    }
    work.parent.state = if cancelled {
        RunState::Cancelled
    } else if exhausted && budget.storage_error().is_none() {
        RunState::Paused
    } else if error.is_some() {
        RunState::Failed
    } else {
        RunState::Completed
    };
    work.parent.finished_at = Some(iso(Utc::now()));
    work.parent.summary = Some(short(&text, 400)).filter(|s| !s.is_empty());
    work.parent.usage.steps = budget.steps();
    work.parent.usage.tool_calls = work.step;
    work.parent.usage.input_tokens = work.usage.input_tokens;
    work.parent.usage.output_tokens = work.usage.output_tokens;
    work.parent.usage.elapsed_ms = work.usage.elapsed_ms;
    let _ = core.inner.store.push_run(&req.session, &work.parent);
    core.emit(AlfaEvent::AgentRunUpdated {
        session_id: sid,
        run: work.parent,
    });
    let reported = crate::conductor_state::delta(work.usage, billed_before);
    let parts = crate::routing::cost_parts(
        core.inner.brain.as_ref(),
        crate::conductor_state::parts_delta(work.by_model, &parts_before),
    );
    let approval = handle.live.lock().ok().and_then(|l| l.approval.clone());
    Outcome {
        text,
        thinking: Vec::new(),
        status,
        stop,
        error,
        usage: (reported.input_tokens + reported.output_tokens > 0).then(|| Usage {
            input_tokens: reported.input_tokens,
            output_tokens: reported.output_tokens,
            ..Usage::default()
        }),
        cost_nano_usd: (reported.cost_nano_usd > 0).then_some(reported.cost_nano_usd),
        chosen: Some(chosen),
        parts,
        latency_ms: millis(started.elapsed()),
        thinking_ms: None,
        tools: work.tools,
        approval,
        local: None,
    }
}
