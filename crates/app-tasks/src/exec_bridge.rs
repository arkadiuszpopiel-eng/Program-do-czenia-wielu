//! Zadanie dla mostu CLI („Delta, zleć to Claude Code"): `agent-backends` z pochodzeniem zadania
//! (`TaskOrigin::launch_origin` — wyzwalacz i Ulepszacz zawsze odmowa, harmonogram tylko ze
//! zgodą per trasa), praca w kopii katalogu roboczego sesji, zdarzenia → Replay oznaczony
//! „niezweryfikowane przez Alfę", granica kroku na końcu każdego narzędzia (steering → `steer`).
//! Sesja „tylko lokalnie" nigdy nie trafia do mostu (dane opuściłyby maszynę).

use std::sync::{Arc, Mutex};

use agent_backends_contract::{
    AgentBackend, AgentEvent, BridgeKind, TaskBudget, TaskId as BridgeTaskId, TaskSpec,
    WorkdirMode, WorkdirSpec,
};
use compliance_contract::SessionTag;
use futures_util::StreamExt;
use scheduler_contract::{
    Dispatch, Steer, StepDirective, StepGate, StepReport, TaskOutput, WorkerResult,
};
use sessions_contract::PrivacyTag;

use crate::bridge_view::BridgeProjector;
use crate::exec_agent::{fail, goal_of, session_of};
use crate::host::ExecDeps;

/// Dopisek wyniku mostu.
pub const UNVERIFIED_NOTE: &str = "_(Wynik mostu CLI — niezweryfikowany przez Alfę.)_";

/// Anuluje zadanie mostu, gdy scheduler przerwie wykonawczynię.
struct CancelOnDrop {
    backend: Arc<dyn AgentBackend>,
    task: Option<BridgeTaskId>,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let (Some(task), Ok(rt)) = (self.task.take(), tokio::runtime::Handle::try_current()) {
            let backend = self.backend.clone();
            rt.spawn(async move {
                // Zadanie zakończone wcześniej = nieznane; nic do anulowania.
                let _ = backend.cancel(&task).await;
            });
        }
    }
}

async fn directive(
    d: StepDirective,
    backend: &dyn AgentBackend,
    task: &BridgeTaskId,
) -> Option<WorkerResult> {
    match d {
        StepDirective::Continue { steering } => {
            for s in steering {
                let text = match s.steer {
                    Steer::Message { text, .. } => text,
                    Steer::ChangeGoal { goal, .. } => format!("Nowy cel od użytkownika: {goal}"),
                    _ => continue,
                };
                if let Err(e) = backend.steer(task, text).await {
                    tracing::warn!(error = %e, "steering mostu nie dotarł");
                }
            }
            None
        }
        StepDirective::Yield { .. } => {
            let _ = backend.cancel(task).await;
            Some(WorkerResult::Yielded)
        }
        StepDirective::Stop { .. } => {
            let _ = backend.cancel(task).await;
            Some(WorkerResult::Stopped)
        }
    }
}

/// Wykonuje zadanie mostu.
pub(crate) async fn run(
    deps: &ExecDeps,
    kind: BridgeKind,
    d: Dispatch,
    gate: Arc<dyn StepGate>,
) -> WorkerResult {
    let Some(backend) = deps.bridges.clone() else {
        return fail("Mosty CLI niepodłączone (moduł agent-backends).", false);
    };
    let session = match session_of(deps, &d).await {
        Ok(s) => s,
        Err(r) => return r,
    };
    let privacy = match deps.host.privacy(&session) {
        PrivacyTag::Normal => SessionTag::Standard,
        PrivacyTag::Private => SessionTag::Private,
        PrivacyTag::LocalOnly => {
            return fail(
                "Sesja „tylko lokalnie” — most CLI wysłałby dane poza komputer.",
                false,
            );
        }
    };
    let Some(workdir) = deps.host.workdir(&session) else {
        return fail(
            "Most CLI pracuje na kopii katalogu roboczego — ustaw katalog roboczy sesji.",
            false,
        );
    };
    let goal = goal_of(&d);
    let spec = TaskSpec {
        bridge: kind,
        prompt: goal.clone(),
        workdir: WorkdirSpec {
            source: workdir.into(),
            mode: WorkdirMode::Worktree,
        },
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        budget: TaskBudget {
            max_turns: Some(d.spec.budget.max_steps),
            wall_clock_ms: Some(d.spec.budget.max_wall_ms),
            max_cost_micro_usd: None,
        },
        session: None,
        alfa_session: session.clone(),
        privacy,
        origin: d.spec.origin.launch_origin(),
        model: None,
    };
    let handle = match backend.submit_task(spec).await {
        Ok(h) => h,
        Err(e) => return fail(format!("Most odmówił: {e}"), false),
    };
    let task = handle.task.clone();
    let _guard = CancelOnDrop {
        backend: backend.clone(),
        task: Some(task.clone()),
    };
    let mut events = match backend.events(&task) {
        Ok(s) => s,
        Err(e) => return fail(format!("zdarzenia mostu: {e}"), true),
    };
    let agent = d.agent.as_ref().map_or("delta", |a| a.as_str()).to_owned();
    let projector = Arc::new(Mutex::new(BridgeProjector::new(
        &session,
        &format!("b{}", task.0),
        d.task.as_str(),
        &agent,
        &goal,
        kind,
    )));
    deps.runs.insert(&task.0, projector.clone());
    let mut early = None;
    let mut result = None;
    while let Some(env) = events.next().await {
        let (projection, run) = {
            let mut p = projector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (p.apply(&env), p.run().clone())
        };
        deps.host.project(&session, &run, projection);
        match &env.event {
            AgentEvent::ToolFinished { .. } if early.is_none() => {
                // Most jest „opaque worker": bez odcisku kroku (pętlę wykrywa samo CLI/budżet).
                let report = StepReport {
                    cost_micro_pln: 0,
                    fingerprint: None,
                };
                early = directive(gate.boundary(report), backend.as_ref(), &task).await;
            }
            AgentEvent::Done { result: r } => {
                result = Some(if r.is_error {
                    fail(format!("Most zakończył z błędem: {}", r.text), false)
                } else {
                    WorkerResult::Succeeded {
                        output: TaskOutput::text(format!("{}\n\n{UNVERIFIED_NOTE}", r.text.trim())),
                    }
                });
            }
            AgentEvent::Error { error } => result = Some(fail(error.to_string(), false)),
            _ => {}
        }
    }
    deps.runs.remove(&task.0);
    early
        .or(result)
        .unwrap_or_else(|| fail("Most zakończył bez wyniku.", true))
}
