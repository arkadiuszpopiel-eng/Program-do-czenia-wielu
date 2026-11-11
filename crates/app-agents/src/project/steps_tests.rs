//! Wynik prośby o zgodę w dzienniku (oś czasu „audit” — dziennik rodzica, plan 50 Z.7).

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use agent_runtime_contract::{RunEvent, RunEventEnvelope, RunId, StepKind, StepStatus};
use app_api::dto::{EventLevel, RunBudgetView, TimelineKind};
use chrono::Utc;
use core_bus_contract::SessionId;
use safety_broker_contract::ApprovalId;

use crate::project::{RunContext, RunProjector};

fn projector() -> RunProjector {
    let ctx = RunContext {
        session: SessionId::new("s1"),
        turn_id: Some("t1".into()),
        agent: "delta".into(),
        role: None,
        run: "r1".into(),
        goal: "zapisz raport".into(),
        workdir: None,
        budget: RunBudgetView {
            max_steps: 10,
            max_minutes: 5,
            max_cost: None,
        },
        broker_window: true,
        approval_timeout_ms: 60_000,
        usd_pln_e4: 40_000,
        started_at: Utc::now(),
        task_id: None,
    };
    RunProjector::new(ctx, BTreeMap::new(), None)
}

fn env(seq: u64, event: RunEvent) -> RunEventEnvelope {
    RunEventEnvelope {
        run: RunId::new("r1"),
        seq,
        at_ms: seq * 10,
        event,
    }
}

fn audit_after(status: StepStatus) -> Vec<(EventLevel, String)> {
    let mut p = projector();
    let start = RunEvent::StepStarted {
        step: 1,
        kind: StepKind::Tool,
        tool: Some("fs.write".into()),
        input: "raport.md".into(),
    };
    p.apply(&env(1, start));
    let wait = RunEvent::WaitingApproval {
        step: 1,
        approval: ApprovalId(7),
        explanation: "zapis poza folderem pracy".into(),
    };
    p.apply(&env(2, wait));
    let finish = RunEvent::StepFinished {
        step: 1,
        kind: StepKind::Tool,
        tool: Some("fs.write".into()),
        status,
        input: "raport.md".into(),
        output: String::new(),
        untrusted: false,
        undo: None,
        intent: None,
        approval: Some(ApprovalId(7)),
    };
    p.apply(&env(3, finish))
        .timeline
        .into_iter()
        .filter(|t| t.kind == TimelineKind::Audit)
        .map(|t| (t.level, t.title))
        .collect()
}

#[test]
fn approved_request_is_logged() {
    let audit = audit_after(StepStatus::Ok);
    assert_eq!(audit.len(), 1, "{audit:?}");
    assert_eq!(audit[0].0, EventLevel::Info);
    assert!(audit[0].1.starts_with("Prośba o zgodę zatwierdzona"));
    assert!(audit[0].1.contains("raport.md"));
}

#[test]
fn unresolved_request_is_logged_as_expired() {
    let audit = audit_after(StepStatus::Denied);
    assert_eq!(audit.len(), 1, "{audit:?}");
    assert_eq!(audit[0].0, EventLevel::Warn);
    assert!(audit[0].1.starts_with("Prośba o zgodę wygasła"));
}

#[test]
fn confirmation_proposal_retains_its_status_in_replay_and_conversation() {
    let mut p = projector();
    let result = p.apply(&env(
        1,
        RunEvent::StepFinished {
            step: 1,
            kind: StepKind::Tool,
            tool: Some("shell_terminal".into()),
            status: StepStatus::NeedsConfirmation,
            input: "Get-Date".into(),
            output: "Zaproponowano, nie wykonano".into(),
            untrusted: false,
            undo: None,
            intent: None,
            approval: None,
        },
    ));
    assert_eq!(
        result.steps[0].status,
        app_api::dto::ReplayStatus::NeedsConfirmation
    );
    assert_eq!(
        p.tool_steps()[0].status,
        app_api::dto::ToolStatus::NeedsConfirmation
    );
    assert!(result.events.iter().any(|e| matches!(e,
        app_api::dto::AlfaEvent::ToolCall { step, .. }
            if step.status == app_api::dto::ToolStatus::NeedsConfirmation)));
}
