//! Projekcja dziennika przebiegu (`agent.*`) na UI: kroki Replay (`AgentStep`), nagłówek
//! przebiegu (`AgentRunUpdated`), linie kroków narzędzi w wątku (`ToolCall`), karta „czeka na
//! zatwierdzenie" (`ApprovalPending`), kapsuła aktywności i wpisy Osi czasu. Czysta maszyna
//! stanów: wejście — koperta zdarzenia, wyjście — zdarzenia UI do wysłania i zapisu.

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_runtime_contract::{RunEvent, RunEventEnvelope, RunOutcome, StepKind};
use app_api::dto::{
    ActivityInfo, AgentRun, AlfaEvent, ApprovalPending, EventLevel, Money, ReplayKind,
    ReplayStatus, ReplayStep, RunBudgetView, RunState, RunUsage, TimelineEvent, TimelineKind,
    ToolStep, iso,
};
use app_api::ids;
use chrono::{DateTime, Utc};
use core_bus_contract::SessionId;
use safety_broker_contract::ApprovalId;

use crate::map::{self, short};
use crate::tickets::TicketLog;

mod steps;

use steps::StepFinish;

/// Stałe fakty przebiegu.
#[derive(Debug, Clone)]
pub struct RunContext {
    /// Sesja.
    pub session: SessionId,
    /// Tura agentki w wątku (`None` — przebieg poza czatem, np. ewaluacja).
    pub turn_id: Option<String>,
    /// Agentka.
    pub agent: String,
    /// Rola agentki w podmiocie Brokera (pierwsza rola z obsady).
    pub role: Option<String>,
    /// Identyfikator przebiegu w `agent-runtime`.
    pub run: String,
    /// Cel.
    pub goal: String,
    /// Katalog roboczy.
    pub workdir: Option<String>,
    /// Budżety (widok UI).
    pub budget: RunBudgetView,
    /// Czy działa okno Brokera.
    pub broker_window: bool,
    /// Limit czekania na zatwierdzenie (ms).
    pub approval_timeout_ms: u64,
    /// Kurs USD→PLN × 10⁴ (koszt przebiegu w groszach).
    pub usd_pln_e4: u64,
    /// Start przebiegu.
    pub started_at: DateTime<Utc>,
}

/// Wynik zastosowania zdarzenia.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Projection {
    /// Zdarzenia do UI (w kolejności).
    pub events: Vec<AlfaEvent>,
    /// Wpisy Osi czasu (do zapisu i `TimelineAppended`).
    pub timeline: Vec<TimelineEvent>,
    /// Kroki zmienione (do zapisu).
    pub steps: Vec<ReplayStep>,
    /// Nagłówek przebiegu zmieniony (do zapisu).
    pub run_changed: bool,
}

/// Projektor jednego przebiegu.
pub struct RunProjector {
    ctx: RunContext,
    titles: BTreeMap<String, String>,
    tickets: Option<Arc<TicketLog>>,
    run: AgentRun,
    steps: BTreeMap<String, ReplayStep>,
    tools: Vec<ToolStep>,
    approvals: BTreeMap<u32, ApprovalId>,
    approval: Option<ApprovalPending>,
    steers: u32,
    last_step: u32,
    outcome: Option<RunOutcome>,
}

fn step_title(titles: &BTreeMap<String, String>, kind: StepKind, tool: Option<&str>) -> String {
    match tool {
        Some(t) => titles.get(t).cloned().unwrap_or_else(|| t.to_owned()),
        None => map::model_step_title(kind).to_owned(),
    }
}

impl RunProjector {
    /// Projektor; `titles` — tytuły narzędzi z manifestów (nazwa → „Zapis pliku").
    pub fn new(
        ctx: RunContext,
        titles: BTreeMap<String, String>,
        tickets: Option<Arc<TicketLog>>,
    ) -> Self {
        let run = AgentRun {
            id: ids::run_dto(&ctx.session, &ctx.run),
            session_id: ctx.session.to_string(),
            turn_id: ctx.turn_id.clone(),
            agent: ctx.agent.clone(),
            goal: ctx.goal.clone(),
            workdir: ctx.workdir.clone(),
            state: RunState::Running,
            started_at: iso(ctx.started_at),
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
            budget: ctx.budget.clone(),
        };
        Self {
            ctx,
            titles,
            tickets,
            run,
            steps: BTreeMap::new(),
            tools: Vec::new(),
            approvals: BTreeMap::new(),
            approval: None,
            steers: 0,
            last_step: 0,
            outcome: None,
        }
    }

    /// Nagłówek przebiegu.
    pub fn run(&self) -> &AgentRun {
        &self.run
    }

    /// Kroki w kolejności numerów.
    pub fn steps(&self) -> Vec<ReplayStep> {
        let mut out: Vec<ReplayStep> = self.steps.values().cloned().collect();
        out.sort_by(|a, b| (a.n, a.at_ms).cmp(&(b.n, b.at_ms)));
        out
    }

    /// Linie kroków narzędzi w wątku.
    pub fn tool_steps(&self) -> &[ToolStep] {
        &self.tools
    }

    /// Karta zatwierdzenia (ostatnia).
    pub fn approval(&self) -> Option<&ApprovalPending> {
        self.approval.as_ref()
    }

    /// Czy agentka czeka teraz na zatwierdzenie.
    pub fn waiting(&self) -> bool {
        self.run.state == RunState::WaitingApproval
    }

    /// Wynik (po `Finished`).
    pub fn outcome(&self) -> Option<&RunOutcome> {
        self.outcome.as_ref()
    }

    fn sid(&self) -> String {
        self.ctx.session.to_string()
    }

    fn timeline(&self, level: EventLevel, title: String, detail: Option<String>) -> TimelineEvent {
        TimelineEvent {
            id: ids::timeline_dto(&self.ctx.session),
            ts: iso(Utc::now()),
            session_id: self.sid(),
            kind: TimelineKind::Tool,
            level,
            agent: Some(self.ctx.agent.clone()),
            title,
            detail,
            cost: None,
            latency_ms: None,
            turn_id: self.ctx.turn_id.clone(),
        }
    }

    fn push_step(&mut self, out: &mut Projection, step: ReplayStep) {
        self.steps.insert(step.id.clone(), step.clone());
        out.events.push(AlfaEvent::AgentStep {
            session_id: self.sid(),
            run_id: self.run.id.clone(),
            step: step.clone(),
        });
        out.steps.push(step);
    }

    fn push_run(&mut self, out: &mut Projection) {
        out.run_changed = true;
        out.events.push(AlfaEvent::AgentRunUpdated {
            session_id: self.sid(),
            run: self.run.clone(),
        });
    }

    fn push_tool(&mut self, out: &mut Projection, step: ToolStep) {
        match self.tools.iter_mut().find(|t| t.id == step.id) {
            Some(existing) => *existing = step.clone(),
            None => self.tools.push(step.clone()),
        }
        if let Some(turn_id) = &self.ctx.turn_id {
            out.events.push(AlfaEvent::ToolCall {
                session_id: self.sid(),
                turn_id: turn_id.clone(),
                step,
            });
        }
    }

    fn push_approval(&mut self, out: &mut Projection, card: ApprovalPending) {
        self.approval = Some(card.clone());
        if let Some(turn_id) = &self.ctx.turn_id {
            out.events.push(AlfaEvent::ApprovalPending {
                session_id: self.sid(),
                turn_id: turn_id.clone(),
                approval: card,
            });
        }
    }

    fn activity(&self, description: String, step: u32) -> AlfaEvent {
        AlfaEvent::ActivityChanged {
            session_id: self.sid(),
            activity: Some(ActivityInfo {
                session_id: self.sid(),
                agent: self.ctx.agent.clone(),
                description,
                step,
                total_steps: self.ctx.budget.max_steps,
                started_at: self.run.started_at.clone(),
            }),
        }
    }

    /// Stosuje zdarzenie przebiegu.
    pub fn apply(&mut self, env: &RunEventEnvelope) -> Projection {
        let mut out = Projection::default();
        match &env.event {
            RunEvent::Started { .. } => {
                self.push_run(&mut out);
                let desc = format!("{}: {}", self.ctx.agent, short(&self.ctx.goal, 60));
                out.events.push(self.activity(desc, 0));
            }
            RunEvent::Planned { text } => {
                let t = self.timeline(
                    EventLevel::Info,
                    format!("Plan: {}", short(text, 120)),
                    None,
                );
                out.timeline.push(t);
            }
            RunEvent::StepStarted {
                step,
                kind,
                tool,
                input,
            } => self.step_started(&mut out, env.at_ms, *step, *kind, tool.as_deref(), input),
            RunEvent::WaitingApproval {
                step,
                approval,
                explanation,
            } => self.waiting_approval(&mut out, *step, *approval, explanation),
            RunEvent::StepFinished {
                step,
                kind,
                tool,
                status,
                output,
                untrusted,
                undo,
                intent,
                ..
            } => {
                let finish = StepFinish {
                    at_ms: env.at_ms,
                    step: *step,
                    kind: *kind,
                    tool: tool.as_deref(),
                    status: *status,
                    output,
                    untrusted: *untrusted,
                    undo: undo.as_ref(),
                    intent: intent.as_ref(),
                };
                self.step_finished(&mut out, &finish);
            }
            RunEvent::Steered { message } => {
                self.steers += 1;
                let step = ReplayStep {
                    id: format!("{}:m{}", self.run.id, self.steers),
                    n: self.last_step,
                    kind: ReplayKind::Steer,
                    tool: None,
                    title: "Wiadomość w trakcie zadania".into(),
                    input: short(message, 200),
                    output: String::new(),
                    status: ReplayStatus::Ok,
                    at_ms: env.at_ms,
                    duration_ms: None,
                    undo_token: None,
                    undone: false,
                    untrusted: false,
                    intent: None,
                    approval_id: None,
                };
                self.push_step(&mut out, step);
            }
            RunEvent::Paused => {
                self.run.state = RunState::Paused;
                self.push_run(&mut out);
            }
            RunEvent::Resumed => {
                self.run.state = RunState::Running;
                self.push_run(&mut out);
            }
            RunEvent::Checkpoint { .. } => {}
            RunEvent::Tainted { source } => {
                let title = format!(
                    "Sesja widziała niezaufaną treść ({source:?}) — ryzykowne akcje wymagają potwierdzenia"
                );
                out.timeline
                    .push(self.timeline(EventLevel::Warn, title, None));
            }
            RunEvent::Usage(u) => {
                let micro_usd = u.cost_nano_usd.div_ceil(1000);
                let micro_pln = u128::from(micro_usd) * u128::from(self.ctx.usd_pln_e4) / 10_000;
                self.run.usage = RunUsage {
                    steps: u.steps,
                    tool_calls: u.tool_calls,
                    input_tokens: u.input_tokens,
                    output_tokens: u.output_tokens,
                    cost: Money::from_micro_pln(u64::try_from(micro_pln).unwrap_or(u64::MAX)),
                    elapsed_ms: u.elapsed_ms,
                };
                self.push_run(&mut out);
            }
            RunEvent::BudgetExceeded { budget } => {
                let title = format!("Przekroczony budżet {} — zatrzymanie", budget.label_pl());
                out.timeline
                    .push(self.timeline(EventLevel::Warn, title, None));
            }
            RunEvent::LoopDetected { tool, repeats } => {
                let title = format!("Wykryta pętla: {tool} ×{repeats} — zatrzymanie");
                out.timeline
                    .push(self.timeline(EventLevel::Warn, title, None));
            }
            RunEvent::Verified { ok, note } => {
                let (level, head) = if *ok {
                    (EventLevel::Info, "Weryfikacja: OK")
                } else {
                    (EventLevel::Warn, "Weryfikacja: problem")
                };
                let t = self.timeline(level, head.to_owned(), Some(short(note, 300)));
                out.timeline.push(t);
            }
            RunEvent::Finished { outcome } => {
                self.outcome = Some(outcome.clone());
                self.run.state = map::run_state(outcome);
                self.run.finished_at = Some(iso(Utc::now()));
                self.run.summary =
                    Some(short(&map::final_text(outcome).text, 400)).filter(|s| !s.is_empty());
                self.push_run(&mut out);
            }
        }
        out
    }
}
