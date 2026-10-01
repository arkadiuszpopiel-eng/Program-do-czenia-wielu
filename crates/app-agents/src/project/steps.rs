//! Kroki przebiegu w projekcji: start kroku, oczekiwanie na zatwierdzenie (karta z biletem
//! Brokera), rozstrzygnięcie zatwierdzenia i koniec kroku (linia w wątku, „Cofnij", Oś czasu).

use agent_runtime_contract::{StepKind, StepStatus};
use app_api::dto::{
    ApprovalPending, ApprovalStatus, EventLevel, ReplayStatus, ReplayStep, RunState, ToolStatus,
    ToolStep, iso,
};
use app_api::ids;
use chrono::Utc;
use risk_classifier_contract::Reversibility;
use safety_broker_contract::{ApprovalId, ApprovalStatus as BrokerApproval, Holder};

use super::{Projection, RunProjector, step_title};
use crate::map::{self, short};

impl RunProjector {
    pub(super) fn step_started(
        &mut self,
        out: &mut Projection,
        at_ms: u64,
        n: u32,
        kind: StepKind,
        tool: Option<&str>,
        input: &str,
    ) {
        self.last_step = self.last_step.max(n);
        let title = step_title(&self.titles, kind, tool);
        let id = ids::step_dto(&self.ctx.session, &self.ctx.run, n);
        let step = ReplayStep {
            id: id.clone(),
            n,
            kind: map::replay_kind(kind),
            tool: tool.map(str::to_owned),
            title: title.clone(),
            input: short(input, 200),
            output: String::new(),
            status: ReplayStatus::Running,
            at_ms,
            duration_ms: None,
            undo_token: None,
            undone: false,
            untrusted: false,
            intent: None,
            approval_id: None,
        };
        self.push_step(out, step);
        if let Some(t) = tool {
            let line = ToolStep {
                id,
                icon: map::tool_icon(t),
                label: title.clone(),
                status: ToolStatus::Running,
                duration_ms: None,
                undo_token: None,
                undone: false,
                intent: None,
            };
            self.push_tool(out, line);
        }
        out.events.push(self.activity(title, n));
    }

    pub(super) fn waiting_approval(
        &mut self,
        out: &mut Projection,
        n: u32,
        id: ApprovalId,
        why: &str,
    ) {
        self.approvals.insert(n, id);
        let step_id = ids::step_dto(&self.ctx.session, &self.ctx.run, n);
        let (title, input) = match self.steps.get_mut(&step_id) {
            Some(step) => {
                step.status = ReplayStatus::WaitingApproval;
                step.approval_id = Some(id.0.to_string());
                (step.title.clone(), step.input.clone())
            }
            None => ("Akcja".to_owned(), String::new()),
        };
        if let Some(step) = self.steps.get(&step_id).cloned() {
            self.push_step(out, step);
        }
        let note = self
            .tickets
            .as_ref()
            .and_then(|t| t.note(id, &self.ctx.session));
        let what = match &note {
            Some(n) => format!("{title} — {}", n.capability),
            None => format!("{title} — {}", short(&input, 120)),
        };
        let expires = self.ctx.started_at
            + chrono::Duration::milliseconds(i64::try_from(self.run.usage.elapsed_ms).unwrap_or(0))
            + chrono::Duration::milliseconds(
                i64::try_from(self.ctx.approval_timeout_ms).unwrap_or(i64::MAX / 2),
            );
        let card = ApprovalPending {
            id: id.0.to_string(),
            what,
            why: note
                .as_ref()
                .map_or_else(|| why.to_owned(), |n| n.explanation.clone()),
            reversible: note
                .as_ref()
                .is_none_or(|n| n.reversible != Reversibility::No),
            risk: note.map_or(app_api::dto::RiskLevel::Medium, |n| map::risk(n.risk)),
            status: ApprovalStatus::Pending,
            broker_window: self.ctx.broker_window,
            expires_at: Some(iso(expires.max(Utc::now()))),
        };
        self.push_approval(out, card);
        self.run.state = RunState::WaitingApproval;
        self.push_run(out);
    }

    fn approval_resolved(&mut self, out: &mut Projection, n: u32, status: StepStatus) {
        let Some(id) = self.approvals.remove(&n) else {
            return;
        };
        let resolved = if status == StepStatus::Ok {
            ApprovalStatus::Approved
        } else {
            let mut holder = Holder::agent(self.ctx.session.as_str(), &self.ctx.agent);
            holder.role.clone_from(&self.ctx.role);
            match self.tickets.as_ref().and_then(|t| t.resolved(id, &holder)) {
                Some(BrokerApproval::Denied) => ApprovalStatus::Denied,
                Some(BrokerApproval::Approved { .. }) => ApprovalStatus::Approved,
                _ => ApprovalStatus::Expired,
            }
        };
        if let Some(mut card) = self.approval.clone().filter(|c| c.id == id.0.to_string()) {
            card.status = resolved;
            self.push_approval(out, card);
        }
        if self.run.state == RunState::WaitingApproval {
            self.run.state = RunState::Running;
            self.push_run(out);
        }
    }

    pub(super) fn step_finished(&mut self, out: &mut Projection, f: &StepFinish<'_>) {
        let step_id = ids::step_dto(&self.ctx.session, &self.ctx.run, f.step);
        let title = step_title(&self.titles, f.kind, f.tool);
        let started = self.steps.get(&step_id).map_or(f.at_ms, |s| s.at_ms);
        let duration = f.at_ms.saturating_sub(started);
        let undo_token = f.undo.map(|u| map::undo_token(&self.ctx.session, u));
        let intent = f.intent.and_then(map::intent);
        let mut step = self.steps.get(&step_id).cloned().unwrap_or(ReplayStep {
            id: step_id.clone(),
            n: f.step,
            kind: map::replay_kind(f.kind),
            tool: f.tool.map(str::to_owned),
            title: title.clone(),
            input: String::new(),
            output: String::new(),
            status: ReplayStatus::Running,
            at_ms: f.at_ms,
            duration_ms: None,
            undo_token: None,
            undone: false,
            untrusted: false,
            intent: None,
            approval_id: None,
        });
        step.status = map::replay_status(f.status);
        step.output = short(f.output, 300);
        step.duration_ms = Some(duration);
        step.undo_token.clone_from(&undo_token);
        step.untrusted = f.untrusted;
        step.intent.clone_from(&intent);
        self.push_step(out, step);
        self.approval_resolved(out, f.step, f.status);
        let Some(tool) = f.tool else {
            return;
        };
        let label = match f.undo {
            Some(u) if !u.text.is_empty() => u.text.clone(),
            _ => format!("{title}: {}", short(f.output, 80)),
        };
        let line = ToolStep {
            id: step_id,
            icon: map::tool_icon(tool),
            label: label.clone(),
            status: map::tool_status(f.status),
            duration_ms: Some(duration),
            undo_token,
            undone: false,
            intent,
        };
        self.push_tool(out, line);
        let level = match f.status {
            StepStatus::Ok | StepStatus::NeedsConfirmation => EventLevel::Info,
            _ => EventLevel::Warn,
        };
        let mut entry = self.timeline(level, format!("Krok {}: {label}", f.step), None);
        entry.detail = Some(short(f.output, 300));
        entry.latency_ms = Some(duration);
        out.timeline.push(entry);
    }
}

/// Koniec kroku (pola zdarzenia `StepFinished`).
pub(super) struct StepFinish<'a> {
    pub(super) at_ms: u64,
    pub(super) step: u32,
    pub(super) kind: StepKind,
    pub(super) tool: Option<&'a str>,
    pub(super) status: StepStatus,
    pub(super) output: &'a str,
    pub(super) untrusted: bool,
    pub(super) undo: Option<&'a tools_common_contract::UndoRef>,
    pub(super) intent: Option<&'a tools_common_contract::ToolIntent>,
}
