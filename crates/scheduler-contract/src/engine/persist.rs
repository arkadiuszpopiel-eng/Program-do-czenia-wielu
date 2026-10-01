//! Trwałość (restart = wznowienie), widoki, najbliższy termin i porządkowanie zakończonych.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use scheduler_lite_contract::Resource;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::engine::{Ctx, SchedCore, State, TaskRec};
use crate::events::{EVENT_RESTORED, global_event};
use crate::host::SchedHost;
use crate::ids::{DispatchId, TaskId};
use crate::roster::{Roster, SystemConditions};
use crate::state::{TaskState, TaskView};
use crate::steer::StopReason;
use crate::validate::{RETENTION_MS, STOP_GRACE_MS};

/// Wersja formatu stanu.
pub const SNAPSHOT_VERSION: u32 = 1;

/// Utrwalany stan schedulera (nieprzezroczysty; JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Wersja formatu.
    pub version: u32,
    /// Numer zmiany stanu (monotoniczny) — nowszy stan ma większy numer.
    #[serde(default)]
    pub revision: u64,
    /// Chwila zapisu (ms).
    pub taken_at_ms: u64,
    state: State,
}

impl Snapshot {
    /// Liczba zadań w stanie.
    pub fn task_count(&self) -> usize {
        self.state.tasks.len()
    }

    /// Liczba zadań niezakończonych.
    pub fn active_count(&self) -> usize {
        self.state
            .tasks
            .values()
            .filter(|r| !r.state.is_terminal())
            .count()
    }
}

impl State {
    pub(crate) fn snapshot(&self, now_ms: u64) -> Snapshot {
        Snapshot {
            version: SNAPSHOT_VERSION,
            revision: self.revision,
            taken_at_ms: now_ms,
            state: self.clone(),
        }
    }
}

fn view(rec: &TaskRec) -> TaskView {
    TaskView {
        spec: rec.spec.clone(),
        state: rec.state.clone(),
        attempt: rec.failures.saturating_add(1),
        steps: rec.steps,
        cost_micro_pln: rec.cost_micro_pln,
        wall_ms: rec.wall_ms,
        preemptions: rec.preemptions,
        blocked: rec.blocked.clone(),
        submitted_at_ms: rec.submitted_at_ms,
        deadline_ms: rec.deadline_ms,
        finished_at_ms: rec.finished_at_ms,
        children: rec.children.clone(),
        pending_steers: rec.steering.len(),
        interrupted: rec.interrupted,
    }
}

impl<H: SchedHost> SchedCore<H> {
    /// Odtwarza stan po restarcie: zadania przerwane w toku wracają do kolejki (`interrupted`,
    /// wznowienie od ukończonych kroków); przerwane z prośbą o zatrzymanie — kończą się.
    /// Dzierżawy nie są odtwarzane (proces zniknął, zasoby są wolne). Sterownik wywołuje potem
    /// `pump()`, żeby odebrać przydziały.
    pub fn restore(host: Arc<H>, snapshot: Snapshot) -> Result<Arc<Self>, String> {
        if snapshot.version != SNAPSHOT_VERSION {
            return Err(format!(
                "nieobsługiwana wersja stanu schedulera: {}",
                snapshot.version
            ));
        }
        let taken = snapshot.taken_at_ms;
        let mut st = snapshot.state;
        let mut stopped: Vec<(TaskId, StopReason)> = Vec::new();
        let mut interrupted = 0usize;
        for (id, rec) in &mut st.tasks {
            if let TaskState::Running { since_ms, .. } = rec.state {
                rec.wall_ms = rec.wall_ms.saturating_add(taken.saturating_sub(since_ms));
                rec.state = TaskState::Ready;
                rec.blocked = None;
                rec.yield_request = None;
                rec.interrupted = true;
                interrupted += 1;
                if let Some(reason) = rec.stop_request.take() {
                    stopped.push((id.clone(), reason));
                }
                rec.stop_requested_at_ms = None;
            }
        }
        let core = Self::with_state(Arc::clone(&host), st);
        let total = core.read(|i| i.st.tasks.len());
        // Bez przeglądu kolejki: przydziały odbierze sterownik pierwszym `pump()`.
        let _ = core.op_with(false, |ctx| {
            for (id, reason) in &stopped {
                ctx.finalize(id, crate::engine::run::termination_for(reason));
            }
            ctx.out.events.push(global_event(
                EVENT_RESTORED,
                ctx.now,
                json!({ "tasks": total, "interrupted": interrupted, "stopped": stopped.len() }),
            ));
            ctx.out.dirty = true;
            Ok(())
        });
        Ok(core)
    }

    /// Bieżący stan do zapisu.
    pub fn snapshot(&self) -> Snapshot {
        let now = self.host().now_ms();
        self.read(|i| i.st.snapshot(now))
    }

    /// Widok zadania.
    pub fn task(&self, id: &TaskId) -> Option<TaskView> {
        self.read(|i| i.st.tasks.get(id).map(view))
    }

    /// Widoki wszystkich zadań (kolejność zgłoszeń).
    pub fn tasks(&self) -> Vec<TaskView> {
        self.read(|i| {
            let mut all: Vec<&TaskRec> = i.st.tasks.values().collect();
            all.sort_by_key(|r| r.seq);
            all.into_iter().map(view).collect()
        })
    }

    /// Zadania w toku z wysłaniami.
    pub fn running(&self) -> Vec<(TaskId, DispatchId)> {
        self.read(|i| {
            i.st.tasks
                .iter()
                .filter_map(|(id, r)| r.running_dispatch().map(|d| (id.clone(), d)))
                .collect()
        })
    }

    /// Zasoby trzymane przez zadania (testy wyłączności, panel Agentki).
    pub fn held_resources(&self) -> BTreeMap<TaskId, Vec<Resource>> {
        self.read(|i| {
            i.leases
                .iter()
                .map(|(id, ls)| {
                    (
                        id.clone(),
                        ls.iter().map(|l| l.resource().clone()).collect(),
                    )
                })
                .collect()
        })
    }

    /// Obsada.
    pub fn roster(&self) -> Roster {
        self.read(|i| i.st.roster.clone())
    }

    /// Warunki systemowe.
    pub fn conditions(&self) -> SystemConditions {
        self.read(|i| i.st.conditions)
    }

    /// Najbliższa chwila, w której stan może się zmienić bez zewnętrznego bodźca (ms).
    pub fn next_wake(&self) -> Option<u64> {
        let now = self.host().now_ms();
        let tasks = self.read(|i| i.st.tasks.values().filter_map(|r| next_for(r, now)).min());
        let lite = self.locks().next_deadline();
        match (tasks, lite) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

fn next_for(rec: &TaskRec, now: u64) -> Option<u64> {
    match &rec.state {
        TaskState::Done { .. } => None,
        TaskState::Running { since_ms, .. } => Some(match rec.stop_requested_at_ms {
            Some(at) => at.saturating_add(STOP_GRACE_MS),
            None => {
                let left = rec.spec.budget.max_wall_ms.saturating_sub(rec.wall_ms);
                rec.deadline_ms.min(since_ms.saturating_add(left))
            }
        }),
        TaskState::RetryWait { until_ms, .. } => Some((*until_ms).min(rec.deadline_ms)),
        TaskState::Ready => Some(
            rec.spec
                .window
                .not_before_ms
                .filter(|t| *t > now)
                .map_or(rec.deadline_ms, |t| t.min(rec.deadline_ms)),
        ),
        TaskState::Pending | TaskState::Paused => Some(rec.deadline_ms),
    }
}

impl<H: SchedHost> Ctx<'_, H> {
    /// Usuwa z pamięci zakończone dawno zadania, do których nic aktywnego się nie odwołuje.
    pub(crate) fn prune(&mut self) {
        let tasks = &self.inner.st.tasks;
        let referenced: BTreeSet<&TaskId> = tasks
            .values()
            .filter(|r| !r.state.is_terminal())
            .flat_map(|r| {
                r.spec
                    .deps
                    .iter()
                    .map(|d| &d.task)
                    .chain(r.spec.parent.as_ref())
            })
            .collect();
        let old: Vec<TaskId> = tasks
            .iter()
            .filter(|(id, r)| {
                r.finished_at_ms
                    .is_some_and(|f| f.saturating_add(RETENTION_MS) <= self.now)
                    && !referenced.contains(id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        if old.is_empty() {
            return;
        }
        for id in &old {
            self.inner.st.tasks.remove(id);
        }
        self.out.dirty = true;
    }
}
