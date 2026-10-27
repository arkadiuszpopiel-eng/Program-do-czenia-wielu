//! Atomowy stan Dyrygentki: kursor zleceń, odpowiedzi, zużycie i powiązanie potomka.
use crate::{ChatEngine, GenRequest};
use agent_runtime_contract::{Checkpoint, RunOutcome, UsageTotals};
use app_api::dto::{AgentRun, ToolStep};
use app_api::{AppError, ids};
use providers_contract::{AssistantTurn, ChatRequest, ContentBlock, Message, ToolUse};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Saved {
    pub request: ChatRequest,
    pub current: AssistantTurn,
    pub base_history: Vec<Message>,
    pub parent: AgentRun,
    pub parent_raw: String,
    pub tools: Vec<ToolStep>,
    pub by_model: BTreeMap<String, (u64, u64, Option<u64>)>,
    pub usage: UsageTotals,
    pub step: u32,
    pub summaries: Vec<String>,
    pub pending: Vec<ToolUse>,
    pub results: Vec<ContentBlock>,
    pub message_added: bool,
    pub awaiting_model: bool,
    pub complete: bool,
    #[serde(default)]
    pub evidence: Vec<crate::conductor_evidence::Receipt>,
}
fn key(turn: &str) -> String {
    format!("conductor-{turn}")
}
pub(crate) fn load(core: &ChatEngine, req: &GenRequest) -> Option<Saved> {
    let turn = ids::turn_dto(&req.session, req.continues?);
    let v = core.inner.store.work(&req.session, &key(&turn)).ok()??;
    let mut saved: Saved = serde_json::from_value(v).ok()?;
    use sessions_contract::SessionCatalog;
    saved.request.meta.privacy.tag =
        match core.inner.sessions.session(&req.session).map(|s| s.privacy) {
            Ok(sessions_contract::PrivacyTag::Normal) => providers_contract::PrivacyTag::Normal,
            _ => providers_contract::PrivacyTag::Private,
        };
    (!saved.complete).then_some(saved)
}
pub(crate) fn save(
    core: &ChatEngine,
    req: &GenRequest,
    turn: &str,
    state: &mut Saved,
) -> Result<(), AppError> {
    let previous_state = core.inner.store.work(&req.session, &key(turn))?;
    if let Some(old) = previous_state.as_ref()
        && let (Some(current), Some(previous)) = (
            state.pending.first_mut(),
            old["pending"].as_array().and_then(|a| a.first()),
        )
        && previous["id"] == current.id
        && let (Some(dst), Some(src)) =
            (current.input.as_object_mut(), previous["input"].as_object())
    {
        for (name, value) in src.iter().filter(|(k, _)| k.starts_with("__")) {
            dst.insert(name.clone(), value.clone());
        }
    }
    if let Some(old) = previous_state {
        let receipts = serde_json::from_value::<Vec<crate::conductor_evidence::Receipt>>(
            old.get("evidence")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
        )
        .map_err(AppError::storage)?;
        crate::conductor_evidence::merge(&mut state.evidence, receipts);
    }
    core.inner.store.save_work(
        &req.session,
        &key(turn),
        "conductor",
        Some(turn),
        &serde_json::to_value(state).map_err(AppError::storage)?,
    )
}
pub(crate) fn update(
    core: &ChatEngine,
    req: &GenRequest,
    turn: &str,
    edit: impl FnOnce(&mut Saved),
) -> Result<(), AppError> {
    if let Some(value) = core.inner.store.work(&req.session, &key(turn))? {
        let mut state: Saved = serde_json::from_value(value).map_err(AppError::storage)?;
        edit(&mut state);
        core.inner.store.save_work(
            &req.session,
            &key(turn),
            "conductor",
            Some(turn),
            &serde_json::to_value(state).map_err(AppError::storage)?,
        )?;
    }
    Ok(())
}
pub(crate) fn calls(turn: &AssistantTurn) -> Vec<ToolUse> {
    turn.message
        .tool_uses()
        .filter(|u| crate::conductor_pipeline::is_conductor_tool(&u.name))
        .cloned()
        .map(|mut u| {
            if let Some(m) = u.input.as_object_mut() {
                m.retain(|key, _| !key.starts_with("__"));
            }
            u
        })
        .collect()
}
pub(crate) fn checkpoint(
    core: &ChatEngine,
    req: &GenRequest,
    turn: &str,
    input: &serde_json::Value,
    goal: &str,
    agent: &str,
    call: &str,
) -> Option<Checkpoint> {
    if let Some(run) = input["__run"].as_str()
        && let Some(value) = core.inner.store.work(&req.session, run).ok()?
    {
        return serde_json::from_value(value).ok();
    }
    let owner = req
        .continues
        .map_or_else(|| turn.to_owned(), |old| ids::turn_dto(&req.session, old));
    core.inner
        .store
        .work_for_turn(&req.session, &owner)
        .ok()?
        .into_iter()
        .filter(|(_, kind, _)| kind == "runtime")
        .filter_map(|(_, _, v)| serde_json::from_value::<Checkpoint>(v).ok())
        .find(|cp| {
            cp.spec.agent.as_str() == agent
                && cp.spec.goal == goal
                && cp.options.owner_call.as_deref() == Some(call)
                && cp.options.parent.is_none()
                && !matches!(
                    cp.finished,
                    Some(RunOutcome::Cancelled | RunOutcome::Refused)
                )
        })
}
pub(crate) fn total(cp: &Checkpoint) -> UsageTotals {
    UsageTotals {
        input_tokens: cp
            .usage
            .input_tokens
            .saturating_add(cp.delegated.input_tokens),
        output_tokens: cp
            .usage
            .output_tokens
            .saturating_add(cp.delegated.output_tokens),
        steps: cp.usage.steps.saturating_add(cp.delegated.steps),
        tool_calls: cp.usage.tool_calls.saturating_add(cp.delegated.tool_calls),
        cost_nano_usd: cp
            .usage
            .cost_nano_usd
            .saturating_add(cp.delegated.cost_nano_usd),
        elapsed_ms: cp.usage.elapsed_ms,
    }
}
pub(crate) fn delta(now: UsageTotals, before: UsageTotals) -> UsageTotals {
    UsageTotals {
        input_tokens: now.input_tokens.saturating_sub(before.input_tokens),
        output_tokens: now.output_tokens.saturating_sub(before.output_tokens),
        steps: now.steps.saturating_sub(before.steps),
        tool_calls: now.tool_calls.saturating_sub(before.tool_calls),
        cost_nano_usd: now.cost_nano_usd.saturating_sub(before.cost_nano_usd),
        elapsed_ms: now.elapsed_ms.saturating_sub(before.elapsed_ms),
    }
}
pub(crate) fn add(to: &mut UsageTotals, u: UsageTotals) {
    to.input_tokens = to.input_tokens.saturating_add(u.input_tokens);
    to.output_tokens = to.output_tokens.saturating_add(u.output_tokens);
    to.steps = to.steps.saturating_add(u.steps);
    to.tool_calls = to.tool_calls.saturating_add(u.tool_calls);
    to.cost_nano_usd = to.cost_nano_usd.saturating_add(u.cost_nano_usd);
}
pub(crate) fn billed(core: &ChatEngine, req: &GenRequest) -> bool {
    req.continues.is_some_and(|turn| {
        core.inner
            .store
            .meta(&req.session, turn)
            .ok()
            .flatten()
            .is_some_and(|m| m.usage.is_some())
    })
}
pub(crate) fn parts_delta(
    now: BTreeMap<String, (u64, u64, Option<u64>)>,
    old: &BTreeMap<String, (u64, u64, Option<u64>)>,
) -> BTreeMap<String, (u64, u64, Option<u64>)> {
    now.into_iter()
        .map(|(key, (a, b, c))| {
            let before = old.get(&key).copied().unwrap_or_default();
            (
                key,
                (
                    a.saturating_sub(before.0),
                    b.saturating_sub(before.1),
                    c.map(|n| n.saturating_sub(before.2.unwrap_or(0))),
                ),
            )
        })
        .filter(|(_, p)| p.0 + p.1 > 0 || p.2.is_some_and(|c| c > 0))
        .collect()
}
pub(crate) fn persist(
    core: &ChatEngine,
    req: &GenRequest,
    turn: &str,
    state: &mut Saved,
    budget: &crate::conductor_budget::ConductorBudget,
    base_elapsed: u64,
) -> bool {
    let limits = budget.limits();
    state.parent.budget.max_steps = limits.max_steps;
    state.parent.budget.max_minutes =
        u32::try_from(limits.max_wall_ms.div_ceil(60_000)).unwrap_or(u32::MAX);
    state.usage.steps = budget.steps();
    state.usage.elapsed_ms = base_elapsed.saturating_add(budget.elapsed_ms());
    if let Err(error) = save(core, req, turn, state) {
        tracing::error!(error=%error,"Nie zapisano stanu Dyrygentki");
        budget.fail_storage(error);
        false
    } else {
        true
    }
}

pub(crate) async fn resume(
    core: &ChatEngine,
    req: &GenRequest,
    handle: &crate::engine::GenHandle,
    saved: Saved,
) -> crate::outcome::Outcome {
    use app_api::ports::BrainRequest;
    use sessions_contract::SessionCatalog;
    let choice = match core
        .inner
        .brain
        .choose(&BrainRequest {
            session: req.session.clone(),
            agent: req.agent.clone(),
            profile: req.profile,
            privacy: core
                .inner
                .sessions
                .session(&req.session)
                .map(|s| s.privacy)
                .unwrap_or_default(),
            chat: Some(saved.request.clone()),
            task: core.model_policy_of(&req.session, &req.agent),
        })
        .await
    {
        Ok(c) => c,
        Err(e) => {
            return crate::outcome::Outcome::failed(app_api::dto::TurnError {
                code: app_api::dto::TurnErrorCode::Provider,
                message: e.to_string(),
                retry_at: None,
                provider: None,
            });
        }
    };
    let chosen = crate::outcome::Chosen {
        provider_id: choice.provider_id.clone(),
        provider_name: choice.provider_name.clone(),
        account: choice.account.clone(),
        model: choice.model.clone(),
    };
    crate::conductor::run_delegated(
        core,
        req,
        handle,
        choice,
        saved.request,
        saved.current,
        chosen,
        std::time::Instant::now(),
        lib_markdown::IncrementalRenderer::new(lib_markdown::RenderOptions::default()),
        0,
        false,
    )
    .await
}

impl Saved {
    pub(crate) fn append_current_message(&mut self) {
        if !self.message_added {
            self.request.messages.push(self.current.message.clone());
            self.message_added = true;
        }
    }
}
