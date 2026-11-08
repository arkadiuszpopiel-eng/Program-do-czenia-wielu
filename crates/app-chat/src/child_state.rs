//! Powiązanie konkretnego zlecenia z trwałym stanem potomka i rozliczeniem przyrostowym.
use crate::conductor_child::ChildExec;
use crate::conductor_state::{self, total};
use agent_runtime_contract::{Checkpoint, RunOutcome, UsageTotals};
use app_api::dto::ToolStep;
use std::collections::BTreeMap;

pub(crate) fn recover(cx: &ChildExec<'_>, goal: &str, agent: &str) -> Option<Checkpoint> {
    let cp =
        conductor_state::checkpoint(cx.core, cx.req, cx.tid, &cx.input, goal, agent, &cx.call_id)?;
    let old = cx
        .req
        .continues
        .map(|id| app_api::ids::turn_dto(&cx.req.session, id));
    if cp.spec.session != cx.req.session
        || cp.spec.agent.as_str() != agent
        || cp.spec.goal != goal
        || cp
            .options
            .owner_turn
            .as_deref()
            .is_none_or(|t| t != cx.tid && Some(t) != old.as_deref())
    {
        return None;
    }
    if cp.options.owner_call.as_deref() != Some(cx.call_id.as_str()) {
        return None;
    }
    matches!(
        cp.finished,
        None | Some(RunOutcome::BudgetExceeded { .. } | RunOutcome::Completed { .. })
    )
    .then_some(cp)
}
pub(crate) fn pipeline_cursor(
    cx: &ChildExec<'_>,
    stages: &[crate::conductor_pipeline::Stage],
    index: usize,
    prev: Option<&str>,
    log: &[String],
    started: bool,
    progress: (u32, &[ToolStep]),
) -> Result<(), app_api::AppError> {
    let (step, tools) = progress;
    conductor_state::update(cx.core, cx.req, cx.tid, |s| {
        s.step = step;
        s.tools = tools.to_vec();
        s.usage.steps = cx.budget.steps();
        if let Some(call) = s.pending.first_mut() {
            call.input["__stages"] = serde_json::to_value(stages).unwrap_or_default();
            call.input["__stage"] = index.into();
            call.input["__prev"] = prev.map_or(serde_json::Value::Null, Into::into);
            call.input["__log"] = serde_json::to_value(log).unwrap_or_default();
            call.input["__stage_started"] = started.into();
            if !started && let Some(map) = call.input.as_object_mut() {
                map.remove("__run");
                map.remove("__accounted");
                map.remove("__accounted_models");
            }
        }
    })
}
pub(crate) fn accounted(cx: &ChildExec<'_>) -> UsageTotals {
    serde_json::from_value(cx.input["__accounted"].clone()).unwrap_or_default()
}
pub(crate) fn bind(cx: &ChildExec<'_>, run: &str) -> Result<(), app_api::AppError> {
    conductor_state::update(cx.core, cx.req, cx.tid, |s| {
        if let Some(call) = s.pending.first_mut() {
            call.input["__run"] = run.into();
        }
    })
}
pub(crate) fn progress(
    cx: &ChildExec<'_>,
    cp: &Checkpoint,
    used: UsageTotals,
    tools: &[ToolStep],
    by_model: &BTreeMap<String, (u64, u64, Option<u64>)>,
) -> Result<(), app_api::AppError> {
    conductor_state::update(cx.core, cx.req, cx.tid, |s| {
        crate::conductor_evidence::merge(
            &mut s.evidence,
            crate::conductor_evidence::capture(
                cp.run.as_str(),
                &cp.spec.persona.name,
                cp.spec.workdir.as_deref(),
                &cp.messages,
            ),
        );
        conductor_state::add(&mut s.usage, used);
        s.usage.elapsed_ms = s.usage.elapsed_ms.saturating_add(used.elapsed_ms);
        s.tools = tools.to_vec();
        s.by_model = by_model.clone();
        if let Some(call) = s.pending.first_mut() {
            call.input["__run"] = cp.run.as_str().into();
            call.input["__accounted"] = serde_json::to_value(total(cp)).unwrap_or_default();
            call.input["__accounted_models"] =
                serde_json::to_value(&cp.model_usage).unwrap_or_default();
        }
    })
}

pub(crate) fn models(
    cx: &ChildExec<'_>,
    cp: &Checkpoint,
    base: &BTreeMap<String, (u64, u64, Option<u64>)>,
) -> BTreeMap<String, (u64, u64, Option<u64>)> {
    let old = serde_json::from_value(cx.input["__accounted_models"].clone()).unwrap_or_default();
    let delta = conductor_state::parts_delta(cp.model_usage.clone(), &old);
    let mut merged = base.clone();
    for (m, (a, b, c)) in delta {
        let p = merged.entry(m).or_default();
        p.0 = p.0.saturating_add(a);
        p.1 = p.1.saturating_add(b);
        if let Some(c) = c {
            p.2 = Some(p.2.unwrap_or(0).saturating_add(c));
        }
    }
    merged
}

pub(crate) fn result(outcome: &RunOutcome, name: &str, role_name: &str) -> Result<String, String> {
    let fin = app_agents::final_text(outcome);
    let text = if fin.text.trim().is_empty() {
        format!("{name} zakończyła podzadanie.")
    } else {
        fin.text
    };
    match fin.error {
        Some(e) => Err(format!("[{name} ({role_name}) — błąd]: {}", e.message)),
        None if matches!(
            outcome,
            RunOutcome::Completed {
                verified: None | Some(true),
                ..
            }
        ) =>
        {
            Ok(format!("[{name} ({role_name})]: {text}"))
        }
        None => Err(format!(
            "[{name} ({role_name}) — podzadanie nieukończone]: {}",
            match outcome {
                RunOutcome::Refused =>
                    "zatrzymano po odmowie działania; wcześniejsze wyniki narzędzi pozostają zapisane"
                        .to_owned(),
                RunOutcome::Cancelled =>
                    "anulowano; nie oznacza to braku wcześniejszego wykonania".to_owned(),
                _ => text,
            }
        )),
    }
}
pub(crate) fn sum_usage(values: impl Iterator<Item = UsageTotals>) -> UsageTotals {
    let mut total = UsageTotals::default();
    for used in values {
        conductor_state::add(&mut total, used);
    }
    total
}

/// Reuse a committed child result without executing it again.
pub(crate) fn completed(
    cx: &ChildExec<'_>,
    cp: &Checkpoint,
    prior: UsageTotals,
    name: &str,
    all_tools: &[ToolStep],
    by_model: &mut BTreeMap<String, (u64, u64, Option<u64>)>,
) -> (Result<String, String>, UsageTotals) {
    let used = crate::conductor_state::delta(crate::conductor_state::total(cp), prior);
    cx.budget.charge(used);
    *by_model = crate::child_state::models(cx, cp, by_model);
    if let Err(e) = crate::child_state::progress(cx, cp, used, all_tools, by_model) {
        cx.budget.fail_storage(&e);
        return (Err(e.to_string()), used);
    }
    (
        result(
            cp.finished.as_ref().unwrap_or(&RunOutcome::Cancelled),
            name,
            "zapisane zlecenie",
        ),
        used,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refusal_and_cancellation_are_not_successful_completion() {
        for outcome in [RunOutcome::Refused, RunOutcome::Cancelled] {
            assert!(result(&outcome, "Delta", "coder").is_err());
        }
    }
}
