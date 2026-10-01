//! Widoki schedulera, wyzwalaczy i Marszałka → DTO panelu Zadania / Ustawień.

use app_api::AppError;
use app_api::dto::{
    Iso8601, MarshalProposalInfo, MarshalRejected, MarshalReport, MarshalRuleInfo, Money,
    TaskClassKind, TaskDep, TaskInfo, TaskOriginKind, TaskResultKind, TaskStateKind, TriggerDraft,
    TriggerInfo, TriggerKindView, TriggerRunInfo, iso,
};
use marshal_contract::{DailyReport, Proposal, ProposalStatus, Rule};
use personas_contract::PersonaId;
use scheduler_contract::{
    Assignee, DepCondition, ExecutorKind, TaskClass, TaskOrigin, TaskState, TaskView, Termination,
};
use triggers_contract::{
    Actor, CronExpr, FinishFilter, RunRecord, TriggerAction, TriggerKind, TriggerSpec, TriggerView,
};

use crate::bridge_view::bridge_name;
use crate::text;

/// Czas (ms UTC) → ISO.
pub fn iso_ms(ms: u64) -> Iso8601 {
    let ms = i64::try_from(ms).unwrap_or(i64::MAX);
    iso(chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default())
}

fn assignee(a: &Assignee) -> String {
    match a {
        Assignee::Persona(p) => p.to_string(),
        Assignee::Role(r) => format!("role:{r}"),
        Assignee::AnyAgent => "any".into(),
        Assignee::System(s) => format!("system:{s}"),
    }
}

/// Wykonawca → DTO.
pub fn executor(e: &ExecutorKind) -> String {
    match e {
        ExecutorKind::Agent => "agent".into(),
        ExecutorKind::Bridge(b) => format!("bridge:{}", bridge_name(*b)),
        ExecutorKind::Service(s) => format!("service:{s}"),
    }
}

fn origin(o: &TaskOrigin) -> (TaskOriginKind, Option<String>) {
    match o {
        TaskOrigin::User => (TaskOriginKind::User, None),
        TaskOrigin::Agent { persona } => (TaskOriginKind::Agent, Some(persona.to_string())),
        TaskOrigin::Trigger { trigger_id, .. } => {
            (TaskOriginKind::Trigger, Some(trigger_id.clone()))
        }
        TaskOrigin::Schedule { schedule_id } => {
            (TaskOriginKind::Schedule, Some(schedule_id.clone()))
        }
        TaskOrigin::Improver => (TaskOriginKind::Improver, None),
        TaskOrigin::System { service } => (TaskOriginKind::System, Some(service.clone())),
    }
}

fn result(t: &Termination) -> TaskResultKind {
    match t {
        Termination::Succeeded { .. } => TaskResultKind::Succeeded,
        Termination::Failed { .. } => TaskResultKind::Failed,
        Termination::Cancelled { .. } => TaskResultKind::Cancelled,
        Termination::Skipped { .. } => TaskResultKind::Skipped,
        Termination::Expired { .. } => TaskResultKind::Expired,
        Termination::BudgetExceeded { .. } => TaskResultKind::BudgetExceeded,
        Termination::BudgetBlocked { .. } => TaskResultKind::BudgetBlocked,
    }
}

fn condition(c: &DepCondition) -> &'static str {
    match c {
        DepCondition::Succeeded => "succeeded",
        DepCondition::Failed => "failed",
        DepCondition::Finished => "finished",
        DepCondition::OutputEquals { .. } => "output_equals",
    }
}

/// Zadanie → DTO.
pub fn task(v: &TaskView) -> TaskInfo {
    let s = &v.spec;
    let (state, agent, error, done) = match &v.state {
        TaskState::Pending => (TaskStateKind::Pending, None, None, None),
        TaskState::Ready => (TaskStateKind::Ready, None, None, None),
        TaskState::Running { agent, .. } => (
            TaskStateKind::Running,
            agent.as_ref().map(PersonaId::to_string),
            None,
            None,
        ),
        TaskState::RetryWait { last_error, .. } => (
            TaskStateKind::RetryWait,
            None,
            Some(last_error.clone()),
            None,
        ),
        TaskState::Paused => (TaskStateKind::Paused, None, None, None),
        TaskState::Done { termination } => (
            TaskStateKind::Done,
            None,
            text::termination(termination),
            Some(termination),
        ),
    };
    let (origin, origin_detail) = origin(&s.origin);
    TaskInfo {
        id: s.id.to_string(),
        title: s.title.clone(),
        parent_id: s.parent.as_ref().map(ToString::to_string),
        deps: s
            .deps
            .iter()
            .map(|d| TaskDep {
                task_id: d.task.to_string(),
                condition: condition(&d.condition).into(),
            })
            .collect(),
        assignee: assignee(&s.assignee),
        agent,
        class: match s.class {
            TaskClass::User => TaskClassKind::User,
            TaskClass::Agent => TaskClassKind::Agent,
            TaskClass::Background => TaskClassKind::Background,
        },
        origin,
        origin_detail,
        executor: executor(&s.executor),
        state,
        result: done.map(result),
        blocked: v.blocked.as_ref().map(text::block),
        error,
        summary: done.and_then(|t| match t {
            Termination::Succeeded { output } => Some(output.summary.clone()),
            _ => None,
        }),
        attempt: v.attempt,
        max_attempts: s.retry.max_attempts,
        steps: v.steps,
        max_steps: s.budget.max_steps,
        cost: Money::from_micro_pln(v.cost_micro_pln),
        session_id: s.session.as_ref().map(ToString::to_string),
        tainted: s.is_tainted(),
        submitted_at: iso_ms(v.submitted_at_ms),
        deadline_at: iso_ms(v.deadline_ms),
    }
}

fn actor(a: &Actor) -> String {
    match a {
        Actor::User => "user".into(),
        Actor::Agent(p) => format!("agent:{p}"),
        Actor::System(s) => format!("system:{s}"),
    }
}

fn kind_view(k: &TriggerKind) -> TriggerKindView {
    match k {
        TriggerKind::Cron { expr } => TriggerKindView::Cron {
            expr: expr.as_str().to_owned(),
        },
        TriggerKind::Once { at_ms } => TriggerKindView::Once { at: iso_ms(*at_ms) },
        TriggerKind::Interval { every_ms, .. } => TriggerKindView::Interval {
            every_minutes: every_ms / 60_000,
        },
        TriggerKind::FileInDir { dir, pattern } => TriggerKindView::FileInDir {
            dir: dir.clone(),
            pattern: pattern.clone(),
        },
        TriggerKind::NewMessage { session } => TriggerKindView::NewMessage {
            session_id: session.as_ref().map(ToString::to_string),
        },
        TriggerKind::TaskFinished {
            task_prefix,
            outcome,
        } => TriggerKindView::TaskFinished {
            task_prefix: task_prefix.clone(),
            outcome: match outcome {
                FinishFilter::Succeeded => "succeeded",
                FinishFilter::Failed => "failed",
                FinishFilter::Any => "any",
            }
            .into(),
        },
        TriggerKind::Manual => TriggerKindView::Manual,
    }
}

/// Wyzwalacz → DTO (`watch` — obserwacja katalogów dostępna).
pub fn trigger(v: &TriggerView, watch: bool) -> TriggerInfo {
    let s = &v.spec;
    TriggerInfo {
        id: s.id.to_string(),
        name: s.name.clone(),
        kind: kind_view(&s.kind),
        enabled: s.enabled,
        owner: actor(&s.owner),
        title: s.action.title.clone(),
        goal: s.action.goal.clone(),
        agent: match &s.action.assignee {
            Assignee::Persona(p) => Some(p.to_string()),
            _ => None,
        },
        bridge: match &s.action.executor {
            ExecutorKind::Bridge(b) => Some(bridge_name(*b).into()),
            _ => None,
        },
        tz: s.tz.to_string(),
        next_fire_at: v.next_fire_ms.map(iso_ms),
        last_fire_at: v.last_fire_ms.map(iso_ms),
        fired: v.fired,
        suppressed: v.suppressed,
        deferred_until: v.deferred_until_ms.map(iso_ms),
        respect_dnd: s.respect_dnd,
        watch_unavailable: !watch && matches!(s.kind, TriggerKind::FileInDir { .. }),
    }
}

/// Wpis dziennika → DTO.
pub fn run(r: &RunRecord) -> TriggerRunInfo {
    let (outcome, task_id, detail) = text::outcome(&r.outcome);
    TriggerRunInfo {
        at: iso_ms(r.at_ms),
        trigger_id: r.trigger.to_string(),
        cause: text::cause(&r.cause),
        outcome: outcome.into(),
        task_id,
        detail,
    }
}

fn parse_ms(at: &str) -> Result<u64, AppError> {
    let t = chrono::DateTime::parse_from_rfc3339(at)
        .map_err(|_| AppError::invalid(format!("Nieprawidłowa data „{at}”.")))?;
    u64::try_from(t.timestamp_millis()).map_err(|_| AppError::invalid("Data sprzed 1970 r."))
}

/// Szkic z UI → specyfikacja (właściciel = użytkownik; most tylko w harmonogramie czasowym).
pub fn spec(id: &str, d: &TriggerDraft) -> Result<TriggerSpec, AppError> {
    let kind = match &d.kind {
        TriggerKindView::Cron { expr } => TriggerKind::Cron {
            expr: CronExpr::parse(expr).map_err(|e| AppError::invalid(format!("Cron: {e}")))?,
        },
        TriggerKindView::Once { at } => TriggerKind::Once {
            at_ms: parse_ms(at)?,
        },
        TriggerKindView::Interval { every_minutes } => TriggerKind::Interval {
            every_ms: every_minutes.saturating_mul(60_000),
            start_ms: None,
        },
        TriggerKindView::FileInDir { dir, pattern } => TriggerKind::FileInDir {
            dir: dir.clone(),
            pattern: pattern.clone().filter(|p| !p.trim().is_empty()),
        },
        TriggerKindView::NewMessage { session_id } => TriggerKind::NewMessage {
            session: session_id.as_deref().map(core_bus_contract::SessionId::new),
        },
        TriggerKindView::TaskFinished {
            task_prefix,
            outcome,
        } => TriggerKind::TaskFinished {
            task_prefix: task_prefix.clone(),
            outcome: match outcome.as_str() {
                "succeeded" => FinishFilter::Succeeded,
                "failed" => FinishFilter::Failed,
                _ => FinishFilter::Any,
            },
        },
        TriggerKindView::Manual => TriggerKind::Manual,
    };
    let mut action = TriggerAction::new(d.title.trim(), d.goal.trim());
    if let Some(agent) = d.agent.as_deref().filter(|a| !a.is_empty()) {
        action.assignee = Assignee::Persona(PersonaId::new(agent));
    }
    let bridge = match d.bridge.as_deref() {
        None | Some("") => None,
        Some("claude_code") => Some(agent_backends_contract::BridgeKind::ClaudeCode),
        Some("codex") => Some(agent_backends_contract::BridgeKind::Codex),
        Some(other) => return Err(AppError::invalid(format!("Nieznany most „{other}”."))),
    };
    let mut spec = TriggerSpec::new(id, d.name.trim(), Actor::User, kind, action);
    if let Some(b) = bridge {
        spec.action.executor = ExecutorKind::Bridge(b);
        spec.action.class = TaskClass::Agent;
        spec.allow_bridges = true;
        spec.rate.max_fires = spec
            .rate
            .max_fires
            .min(triggers_contract::MAX_BRIDGE_FIRES_PER_DAY);
        spec.rate.per_ms = triggers_contract::DAY_MS;
    }
    spec.respect_dnd = d.respect_dnd;
    Ok(spec)
}

/// Reguła → DTO.
pub fn rule(r: &Rule) -> MarshalRuleInfo {
    MarshalRuleInfo {
        id: r.id.to_string(),
        description: r.description.clone(),
        when: text::when(r),
        effects: r.then.iter().map(text::effect).collect(),
        rule: serde_json::to_value(r).unwrap_or_default(),
    }
}

/// Propozycja → DTO.
pub fn proposal(p: &Proposal) -> MarshalProposalInfo {
    MarshalProposalInfo {
        id: p.id,
        text: p.text.clone(),
        rules: p.rules.iter().map(rule).collect(),
        rejected: p
            .rejected
            .iter()
            .map(|r| MarshalRejected {
                draft: r.draft.clone(),
                errors: r.errors.clone(),
            })
            .collect(),
        conflicts: p.conflicts.iter().map(|c| c.message.clone()).collect(),
        status: match p.status {
            ProposalStatus::Pending => "pending",
            ProposalStatus::Approved => "approved",
            ProposalStatus::Rejected => "rejected",
        }
        .into(),
        created_at: iso_ms(p.created_at_ms),
    }
}

/// Raport dzienny → DTO.
pub fn report(r: &DailyReport) -> MarshalReport {
    MarshalReport {
        day: r.day.map(|d| d.to_string()).unwrap_or_default(),
        submitted: u64::from(r.submitted),
        succeeded: u64::from(r.succeeded),
        failed: u64::from(r.failed),
        escalations: u64::from(r.escalations),
        text: r.text.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_become_user_specs_and_bridges_need_time_triggers() {
        let draft = TriggerDraft {
            name: "Poranny raport".into(),
            kind: TriggerKindView::Cron {
                expr: "0 8 * * 1-5".into(),
            },
            title: "Raport".into(),
            goal: "Przygotuj raport".into(),
            agent: Some("beta".into()),
            bridge: Some("claude_code".into()),
            respect_dnd: true,
        };
        let s = spec("poranny-raport", &draft).unwrap();
        assert!(s.allow_bridges && s.owner == Actor::User);
        assert!(s.rate.max_fires <= triggers_contract::MAX_BRIDGE_FIRES_PER_DAY);
        let bad = TriggerDraft {
            kind: TriggerKindView::Cron {
                expr: "nie cron".into(),
            },
            ..draft.clone()
        };
        assert!(spec("x", &bad).is_err());
        let v = kind_view(&s.kind);
        assert_eq!(
            v,
            TriggerKindView::Cron {
                expr: "0 8 * * 1-5".into()
            }
        );
        assert_eq!(iso_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(executor(&ExecutorKind::Service("x".into())), "service:x");
    }
}
