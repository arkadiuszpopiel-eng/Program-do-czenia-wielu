//! Zapis kroku delegacji w Replay i w turze czatu.
use crate::engine::ChatEngine;
use agent_runtime_contract::DELEGATE_TOOL;
use app_agents::short;
use app_api::dto::{AlfaEvent, ReplayKind, ReplayStatus, ReplayStep, ToolStatus, ToolStep};
use sessions_contract::SessionId;

#[allow(clippy::too_many_arguments)]
pub(crate) fn record_delegation_step(
    core: &ChatEngine,
    session: &SessionId,
    run_id: &str,
    tid: &str,
    step_id: String,
    n: u32,
    title: String,
    goal: &str,
    txt: &str,
    err: bool,
    at_ms: u64,
    dur: u64,
    all_tools: &mut [ToolStep],
) {
    let (st, rst) = if err {
        (ToolStatus::Error, ReplayStatus::Failed)
    } else {
        (ToolStatus::Done, ReplayStatus::Ok)
    };
    if let Some(existing) = all_tools.iter_mut().find(|t| t.id == step_id) {
        existing.status = st;
        existing.duration_ms = Some(dur);
        core.emit(AlfaEvent::ToolCall {
            session_id: session.to_string(),
            turn_id: tid.to_owned(),
            step: existing.clone(),
        });
    }
    let rstep = ReplayStep {
        id: step_id,
        n,
        kind: ReplayKind::Tool,
        tool: Some(DELEGATE_TOOL.to_owned()),
        title,
        input: short(goal, 200),
        output: short(txt, 300),
        status: rst,
        at_ms,
        duration_ms: Some(dur),
        undo_token: None,
        undone: false,
        untrusted: false,
        intent: None,
        approval_id: None,
    };
    let _ = core.inner.store.push_step(session, run_id, &rstep);
    core.emit(AlfaEvent::AgentStep {
        session_id: session.to_string(),
        run_id: run_id.to_owned(),
        step: rstep,
    });
}

/// Nagłówek wspólnego przebiegu Dyrygentki.
pub(crate) fn parent(
    req: &crate::generate::GenRequest,
    id: String,
    tid: &str,
    goal: String,
    workdir: &str,
    settings: &app_agents::AgentSettings,
) -> app_api::dto::AgentRun {
    use app_api::dto::{AgentRun, Money, RunState, RunUsage, iso};
    use chrono::Utc;
    AgentRun {
        id,
        session_id: req.session.to_string(),
        turn_id: Some(tid.to_owned()),
        agent: req.agent.clone(),
        goal,
        workdir: Some(workdir.to_owned()),
        state: RunState::Running,
        started_at: iso(Utc::now()),
        finished_at: None,
        summary: None,
        usage: RunUsage {
            steps: 0,
            tool_calls: 0,
            input_tokens: 0,
            output_tokens: 0,
            cost: Money::pln(0),
            elapsed_ms: 0,
        },
        budget: settings.view(),
        bridge: None,
        task_id: None,
        parent_id: None,
        label: Some("Dyrygentka — rozdział zadań".into()),
    }
}
