//! Most CLI w Replay: zdarzenia `agent-backends` → przebieg (`AgentRun.bridge` = „niezweryfikowane
//! przez Alfę") i kroki, w tym krok „czeka na zatwierdzenie" dla prośby o uprawnienie
//! (kanał zatwierdzeń: [`crate::sink`]).

use std::collections::BTreeMap;

use agent_backends_contract::{AgentEvent, AgentEventEnvelope, BridgeKind, PermissionRequest};
use app_agents::{Projection, short};
use app_api::dto::{
    AgentRun, AlfaEvent, Money, ReplayKind, ReplayStatus, ReplayStep, RunBudgetView, RunState,
    RunUsage, iso,
};
use app_api::ids;
use core_bus_contract::SessionId;

/// Nazwa mostu w DTO.
pub fn bridge_name(kind: BridgeKind) -> &'static str {
    match kind {
        BridgeKind::ClaudeCode => "claude_code",
        BridgeKind::Codex => "codex",
    }
}

/// Projektor przebiegu mostu.
pub struct BridgeProjector {
    session: SessionId,
    run: AgentRun,
    steps: Vec<ReplayStep>,
    calls: BTreeMap<String, usize>,
    started_ms: Option<u64>,
}

impl BridgeProjector {
    /// Nowy przebieg mostu dla zadania `task` (przebieg `run`).
    pub fn new(
        session: &SessionId,
        run: &str,
        task: &str,
        agent: &str,
        goal: &str,
        kind: BridgeKind,
    ) -> Self {
        Self {
            session: session.clone(),
            run: AgentRun {
                id: ids::run_dto(session, run),
                session_id: session.to_string(),
                turn_id: None,
                agent: agent.to_owned(),
                goal: short(goal, 200),
                workdir: None,
                state: RunState::Running,
                started_at: iso(chrono::Utc::now()),
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
                budget: RunBudgetView {
                    max_steps: 0,
                    max_minutes: 0,
                    max_cost: None,
                },
                bridge: Some(bridge_name(kind).to_owned()),
                task_id: Some(task.to_owned()),
            },
            steps: Vec::new(),
            calls: BTreeMap::new(),
            started_ms: None,
        }
    }

    /// Nagłówek przebiegu.
    pub fn run(&self) -> &AgentRun {
        &self.run
    }

    /// Sesja przebiegu.
    pub fn session(&self) -> &SessionId {
        &self.session
    }

    fn step(
        &mut self,
        kind: ReplayKind,
        tool: Option<String>,
        title: String,
        input: String,
        status: ReplayStatus,
        at_ms: u64,
    ) -> usize {
        let n = u32::try_from(self.steps.len() + 1).unwrap_or(u32::MAX);
        let run_key = self
            .run
            .id
            .rsplit_once(":r")
            .map_or("", |(_, r)| r)
            .to_owned();
        self.steps.push(ReplayStep {
            id: ids::step_dto(&self.session, &run_key, n),
            n,
            kind,
            tool,
            title,
            input,
            output: String::new(),
            status,
            at_ms,
            duration_ms: None,
            undo_token: None,
            undone: false,
            untrusted: true,
            intent: None,
            approval_id: None,
        });
        self.run.usage.steps = n;
        self.steps.len() - 1
    }

    fn emit(&self, p: &mut Projection, i: usize) {
        let step = self.steps[i].clone();
        p.events.push(AlfaEvent::AgentStep {
            session_id: self.session.to_string(),
            run_id: self.run.id.clone(),
            step: step.clone(),
        });
        p.steps.push(step);
    }

    fn header(&self, p: &mut Projection) {
        p.run_changed = true;
        p.events.push(AlfaEvent::AgentRunUpdated {
            session_id: self.session.to_string(),
            run: self.run.clone(),
        });
    }

    /// Zdarzenie mostu → projekcja (wszystko oznaczone jako niezweryfikowane przez Alfę).
    pub fn apply(&mut self, env: &AgentEventEnvelope) -> Projection {
        let mut p = Projection::default();
        let at = env.at_ms;
        let start = *self.started_ms.get_or_insert(at);
        self.run.usage.elapsed_ms = at.saturating_sub(start);
        match &env.event {
            AgentEvent::Started {
                cli_version,
                workdir,
                ..
            } => {
                self.run.workdir = Some(workdir.to_string_lossy().into_owned());
                let i = self.step(
                    ReplayKind::Plan,
                    None,
                    format!("Start CLI {cli_version}"),
                    String::new(),
                    ReplayStatus::Ok,
                    at,
                );
                self.emit(&mut p, i);
                self.header(&mut p);
            }
            AgentEvent::Plan { items } => {
                let text: Vec<String> = items.iter().map(|i| i.text.clone()).collect();
                let i = self.step(
                    ReplayKind::Plan,
                    None,
                    "Plan".into(),
                    short(&text.join("; "), 300),
                    ReplayStatus::Ok,
                    at,
                );
                self.emit(&mut p, i);
            }
            AgentEvent::Step { text } => {
                let i = self.step(
                    ReplayKind::Think,
                    None,
                    short(text, 120),
                    String::new(),
                    ReplayStatus::Ok,
                    at,
                );
                self.emit(&mut p, i);
            }
            AgentEvent::ToolRequest {
                call_id,
                tool,
                input,
            } => {
                self.run.usage.tool_calls += 1;
                let i = self.step(
                    ReplayKind::Tool,
                    Some(tool.clone()),
                    tool.clone(),
                    short(&input.to_string(), 300),
                    ReplayStatus::Running,
                    at,
                );
                self.calls.insert(call_id.clone(), i);
                self.emit(&mut p, i);
            }
            AgentEvent::ToolFinished {
                call_id,
                is_error,
                preview,
            } => {
                if let Some(&i) = self.calls.get(call_id) {
                    let s = &mut self.steps[i];
                    s.status = if *is_error {
                        ReplayStatus::Failed
                    } else {
                        ReplayStatus::Ok
                    };
                    s.output = short(preview, 300);
                    s.duration_ms = Some(at.saturating_sub(s.at_ms));
                    self.emit(&mut p, i);
                }
            }
            AgentEvent::Usage { usage, .. } => {
                self.run.usage.input_tokens += usage.input_tokens;
                self.run.usage.output_tokens += usage.output_tokens;
                self.header(&mut p);
            }
            AgentEvent::Output {
                text,
                partial: false,
                ..
            } => {
                self.run.summary = Some(short(text, 400));
            }
            AgentEvent::Done { result } => {
                self.run.state = if result.is_error {
                    RunState::Failed
                } else {
                    RunState::Completed
                };
                self.run.summary = Some(short(&result.text, 400));
                self.run.finished_at = Some(iso(chrono::Utc::now()));
                self.header(&mut p);
            }
            AgentEvent::Error { error } => {
                self.run.state = RunState::Failed;
                self.run.summary = Some(error.to_string());
                self.run.finished_at = Some(iso(chrono::Utc::now()));
                self.header(&mut p);
            }
            _ => {}
        }
        p
    }

    /// Prośba o uprawnienie: krok „czeka na zatwierdzenie" (z kartą Brokera, gdy jest).
    pub fn permission(
        &mut self,
        request: &PermissionRequest,
        approval: Option<String>,
    ) -> Projection {
        let mut p = Projection::default();
        let at = u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0);
        let title = format!("Prośba mostu: {}", request.tool);
        let i = self.step(
            ReplayKind::Tool,
            Some(request.tool.clone()),
            title,
            short(&request.input.to_string(), 300),
            ReplayStatus::WaitingApproval,
            at,
        );
        self.steps[i].approval_id = approval;
        self.run.state = RunState::WaitingApproval;
        self.emit(&mut p, i);
        self.header(&mut p);
        p
    }

    /// Decyzja w sprawie ostatniej prośby.
    pub fn resolved(&mut self, allowed: bool) -> Projection {
        let mut p = Projection::default();
        if let Some(i) = self
            .steps
            .iter()
            .rposition(|s| s.status == ReplayStatus::WaitingApproval)
        {
            self.steps[i].status = if allowed {
                ReplayStatus::Ok
            } else {
                ReplayStatus::Denied
            };
            self.emit(&mut p, i);
        }
        self.run.state = RunState::Running;
        self.header(&mut p);
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_backends_contract::{TaskId, TaskResult};

    fn env(seq: u64, event: AgentEvent) -> AgentEventEnvelope {
        AgentEventEnvelope {
            task: TaskId("b1".into()),
            seq,
            at_ms: 1_000 + seq,
            unverified_by_alfa: true,
            event,
        }
    }

    #[test]
    fn bridge_events_become_unverified_replay() {
        let s = SessionId::new("s1");
        let mut p = BridgeProjector::new(
            &s,
            "b1",
            "t1",
            "delta",
            "Popraw testy",
            BridgeKind::ClaudeCode,
        );
        let call = AgentEvent::ToolRequest {
            call_id: "c1".into(),
            tool: "Edit".into(),
            input: serde_json::json!({"file": "a.rs"}),
        };
        let first = p.apply(&env(1, call));
        assert_eq!(first.steps.len(), 1);
        assert!(first.steps[0].untrusted && first.steps[0].status == ReplayStatus::Running);
        let done = p.apply(&env(
            2,
            AgentEvent::ToolFinished {
                call_id: "c1".into(),
                is_error: false,
                preview: "ok".into(),
            },
        ));
        assert_eq!(done.steps[0].status, ReplayStatus::Ok);
        let result = TaskResult {
            text: "Gotowe".into(),
            is_error: false,
            subtype: None,
            session: None,
            num_turns: None,
            duration_ms: None,
        };
        let end = p.apply(&env(3, AgentEvent::Done { result }));
        assert!(end.run_changed);
        assert_eq!(p.run().state, RunState::Completed);
        assert_eq!(p.run().bridge.as_deref(), Some("claude_code"));
        assert_eq!(p.run().task_id.as_deref(), Some("t1"));
        assert_eq!(bridge_name(BridgeKind::Codex), "codex");
    }
}
