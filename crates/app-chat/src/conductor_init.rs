//! Początkowy i odtworzony stan wspólnej orkiestracji.
use crate::conductor_budget::ConductorBudget;
use crate::conductor_state::Saved;
use crate::{ChatEngine, GenHandle, GenRequest};
use agent_runtime_contract::UsageTotals;
use app_api::{dto::AlfaEvent, ids, ports::BrainChoice};
use providers_contract::{AssistantTurn, ChatRequest};
use std::{collections::BTreeMap, time::Instant};
pub(crate) struct Init<'a> {
    pub core: &'a ChatEngine,
    pub req: &'a GenRequest,
    pub handle: &'a GenHandle,
    pub choice: &'a BrainChoice,
    pub request: ChatRequest,
    pub current_turn: AssistantTurn,
    pub model: &'a str,
    pub started: Instant,
}
pub(crate) struct Ready {
    pub work: Saved,
    pub budget: ConductorBudget,
    pub billed_before: UsageTotals,
    pub parts_before: BTreeMap<String, (u64, u64, Option<u64>)>,
    pub base_elapsed: u64,
}
pub(crate) async fn prepare(input: Init<'_>) -> Ready {
    let Init {
        core,
        req,
        handle,
        choice,
        request,
        current_turn,
        model,
        started,
    } = input;
    let sid = req.session.to_string();
    let tid = ids::turn_dto(&req.session, handle.turn);
    let workdir = core.session_workdir(&req.session).unwrap_or_default();
    let mut base_history = request.messages.clone();
    let goal_text = base_history
        .pop()
        .map(|m| m.visible_text())
        .unwrap_or_default();
    let settings = core.agent_settings().await;
    let parent_run_raw = format!("c-{}", handle.turn.0);
    let parent_run_id = ids::run_dto(&req.session, &parent_run_raw);
    let parent_run = crate::conductor_record::parent(
        req,
        parent_run_id.clone(),
        &tid,
        goal_text,
        &workdir,
        &settings,
    );
    let _ = core.inner.store.push_run(&req.session, &parent_run);
    core.emit(AlfaEvent::AgentRunUpdated {
        session_id: sid.clone(),
        run: parent_run.clone(),
    });
    let restored = crate::conductor_state::load(core, req);
    let resuming = restored.is_some();
    let initial_cost = current_turn
        .model
        .as_deref()
        .and_then(|m| choice.provider.cost(m, &current_turn.usage))
        .or_else(|| choice.provider.cost(model, &current_turn.usage))
        .map_or(0, |c| c.nano_usd);
    let usage = agent_runtime_contract::UsageTotals {
        steps: 1,
        input_tokens: current_turn.usage.total_input(),
        output_tokens: current_turn.usage.output_tokens,
        cost_nano_usd: initial_cost,
        ..Default::default()
    };
    let pending = crate::conductor_state::calls(&current_turn);
    let mut work = restored.unwrap_or(crate::conductor_state::Saved {
        request,
        current: current_turn,
        base_history,
        parent: parent_run,
        parent_raw: parent_run_raw,
        tools: Vec::new(),
        by_model: BTreeMap::new(),
        usage,
        step: 0,
        summaries: Vec::new(),
        pending,
        results: Vec::new(),
        message_added: false,
        awaiting_model: false,
        complete: false,
        evidence: Vec::new(),
    });
    work.parent.turn_id = Some(tid.clone());
    let already_billed = resuming && crate::conductor_state::billed(core, req);
    let billed_before = if already_billed {
        work.usage
    } else {
        Default::default()
    };
    let parts_before = if already_billed {
        work.by_model.clone()
    } else {
        BTreeMap::new()
    };
    let base_elapsed = if resuming { work.usage.elapsed_ms } else { 0 };
    let mut limits = settings.budget(u64::from(core.inner.costs.current_rate().rate_e4));
    if resuming {
        limits.max_steps = limits.max_steps.saturating_add(work.usage.steps);
        limits.max_tokens = limits.max_tokens.saturating_add(work.usage.tokens());
    }
    let budget =
        ConductorBudget::new(limits, started).with_policy(core.continuation_policy().await);
    budget.charge(work.usage);
    Ready {
        work,
        budget,
        billed_before,
        parts_before,
        base_elapsed,
    }
}
