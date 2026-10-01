//! Adapter `TaskExecutor` dla schedulera (F5, docs/modules/scheduler/SPEC.md): zadanie z
//! ładunkiem [`AgentTaskPayload`] wykonuje pętla agentki. Granica kroku = `StepGate::boundary`
//! po każdym kroku atomowym (steering, oddanie przy pauzie/wywłaszczeniu, zatrzymanie);
//! oddane zadanie wznawia się z checkpointu (ten sam przebieg). Agentka przydzielona przez
//! scheduler zastępuje tę z ładunku (dane z obsady), budżety zawężone do budżetu zadania,
//! taint i pochodzenie dziedziczone (zadanie spoza użytkownika = polecenie agentki).

use std::sync::Arc;

use agent_runtime_contract::{AgentTaskPayload, Checkpoint, RunId, RunOutcome, RunSpec};
use async_trait::async_trait;
use personas_contract::{PersonaId, Role};
use risk_classifier_contract::CommandOrigin;
use scheduler_contract::{
    Dispatch, Holder as LockHolder, StepGate, TaskExecutor, TaskId, TaskOrigin, TaskOutput,
    WorkerResult,
};

use crate::Runtime;
use crate::handle::RunHandle;
use crate::shared::{Exit, Hooks, Shared};

/// Identyfikator przebiegu zadania (próba = osobny przebieg; wznowienie = ten sam).
pub fn task_run_id(task: &TaskId, attempt: u32) -> RunId {
    let clean: String = task
        .as_str()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    RunId::new(format!("task-{clean}-a{attempt}"))
}

/// Wykonawczyni zadań schedulera na runtime agentek.
pub struct RuntimeExecutor {
    shared: Arc<Shared>,
    usd_pln_e4: u64,
}

impl RuntimeExecutor {
    /// Adapter nad runtime; `usd_pln_e4` = kurs USD→PLN × 10⁴ (koszt kroku dla schedulera).
    pub fn new(runtime: &Runtime, usd_pln_e4: u64) -> Self {
        Self {
            shared: runtime.shared(),
            usd_pln_e4,
        }
    }
}

/// Anuluje przebieg, gdy scheduler porzuci wykonanie (przerwanie siłą po `STOP_GRACE_MS`).
struct CancelOnDrop(Option<Arc<RunHandle>>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(h) = self.0.take() {
            h.cancel.cancel();
        }
    }
}

/// Dopasowanie ładunku do wysłania: agentka, role, budżet, taint, pochodzenie.
pub fn adapt_payload(
    payload: AgentTaskPayload,
    dispatch: &Dispatch,
) -> Result<AgentTaskPayload, String> {
    let AgentTaskPayload {
        mut spec,
        mut options,
    } = payload;
    if let Some(agent) = &dispatch.agent
        && agent.as_str() != spec.agent.as_str()
    {
        swap_persona(&mut spec, &options, agent)?;
    }
    let tb = &dispatch.spec.budget;
    spec.budget.max_steps = spec.budget.max_steps.min(tb.max_steps);
    spec.budget.max_wall_ms = spec.budget.max_wall_ms.min(tb.max_wall_ms);
    if options.inherited_taint.is_none() {
        options.inherited_taint = dispatch.spec.taint.first().cloned();
    }
    if dispatch.spec.origin != TaskOrigin::User && spec.origin != CommandOrigin::UntrustedContent {
        spec.origin = CommandOrigin::Agent;
    }
    if dispatch.spec.is_tainted() && options.label.is_none() {
        options.label = Some("zadanie z niezaufanej treści".into());
    }
    Ok(AgentTaskPayload { spec, options })
}

fn swap_persona(
    spec: &mut RunSpec,
    options: &agent_runtime_contract::RunOptions,
    agent: &PersonaId,
) -> Result<(), String> {
    let crew = options
        .crew
        .as_ref()
        .ok_or_else(|| format!("przydzielono {agent}, a ładunek nie ma obsady"))?;
    let persona = crew
        .persona(agent)
        .ok_or_else(|| format!("nieznana persona {agent}"))?;
    let roles: Vec<Role> = crew
        .cast
        .roles_of(agent)
        .iter()
        .filter_map(|r| crew.role(r).cloned())
        .collect();
    spec.persona = persona.clone();
    spec.agent = core_bus_contract::AgentId::new(agent.as_str());
    spec.roles = roles;
    Ok(())
}

/// Wynik przebiegu → wynik dla schedulera.
pub(crate) fn worker_result(run: &RunId, exit: Exit) -> WorkerResult {
    let outcome = match exit {
        Exit::Yielded => return WorkerResult::Yielded,
        Exit::Stopped(_) => return WorkerResult::Stopped,
        Exit::Finished(o) => o,
    };
    match outcome {
        RunOutcome::Completed {
            verified: Some(false),
            summary,
        } => WorkerResult::Failed {
            error: format!(
                "Krytyczka nie potwierdziła wyniku: {}",
                summary.chars().take(200).collect::<String>()
            ),
            retryable: false,
        },
        RunOutcome::Completed { summary, verified } => WorkerResult::Succeeded {
            output: TaskOutput::text(summary)
                .with("run", serde_json::json!(run))
                .with("verified", serde_json::json!(verified)),
        },
        RunOutcome::Cancelled => WorkerResult::Stopped,
        RunOutcome::Failed { error } => WorkerResult::Failed {
            error,
            retryable: true,
        },
        other => WorkerResult::Failed {
            error: format!("{other:?}"),
            retryable: false,
        },
    }
}

#[async_trait]
impl TaskExecutor for RuntimeExecutor {
    async fn execute(&self, dispatch: Dispatch, gate: Arc<dyn StepGate>) -> WorkerResult {
        let payload =
            match serde_json::from_value::<AgentTaskPayload>(dispatch.spec.payload.clone()) {
                Ok(p) => p,
                Err(e) => {
                    return WorkerResult::Failed {
                        error: format!("niepoprawny ładunek zadania agentki: {e}"),
                        retryable: false,
                    };
                }
            };
        let AgentTaskPayload { spec, options } = match adapt_payload(payload, &dispatch) {
            Ok(p) => p,
            Err(error) => {
                return WorkerResult::Failed {
                    error,
                    retryable: false,
                };
            }
        };
        if let Err(e) = spec.validate() {
            return WorkerResult::Failed {
                error: e,
                retryable: false,
            };
        }
        let run = task_run_id(&dispatch.task, dispatch.attempt);
        let cp = match self.shared.store.latest(&run) {
            Ok(Some(cp)) => match cp.finished.clone() {
                Some(o) => return worker_result(&run, Exit::Finished(o)),
                None => cp,
            },
            _ => Checkpoint::with_options(run.clone(), spec, options),
        };
        let holder = match &dispatch.agent {
            Some(p) => LockHolder::Persona(p.clone()),
            None => LockHolder::System(format!("task:{}", dispatch.task)),
        };
        let hooks = Hooks {
            gate: Some(gate),
            lease_holder: Some(holder),
            usd_pln_e4: self.usd_pln_e4,
            initial_steering: dispatch.steering.clone(),
        };
        let (handle, join) = match self.shared.launch(cp, hooks, None) {
            Ok(x) => x,
            Err(e) => {
                return WorkerResult::Failed {
                    error: e.to_string(),
                    retryable: true,
                };
            }
        };
        let mut guard = CancelOnDrop(Some(handle));
        let exit = join.await.unwrap_or(Exit::Finished(RunOutcome::Failed {
            error: "pętla przebiegu przerwana".into(),
        }));
        guard.0 = None;
        worker_result(&run, exit)
    }
}
