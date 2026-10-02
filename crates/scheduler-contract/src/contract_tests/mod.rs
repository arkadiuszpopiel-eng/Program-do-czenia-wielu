//! Współdzielone testy kontraktowe (feature `contract-tests`) — te same dla `-impl` i `-fake`.
//! Czas steruje [`Harness::advance`] (fake: wirtualny zegar; impl: zatrzymany zegar tokio),
//! wykonawczynie są skryptowane ([`Script`]).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use async_trait::async_trait;

use crate::{
    Assignee, Dispatch, Resource, Scheduler, SteerEnvelope, TaskClass, TaskId, TaskOrigin,
    TaskSpec, TaskState, Termination,
};

mod control;
mod dag;
mod resources;
mod script;
mod time;
mod view;

pub use control::{kill_switch_cancels_everything, restart_resumes, steering_within_one_step};
pub use dag::{cancel_subtree, dag_with_conditions_and_inputs};
pub use resources::{exclusive_resources_in_parallel, priorities_and_preemption, voice_first};
pub use script::{Script, ScriptOutcome, ScriptRun, ScriptedExecutor};
pub use time::{budgets, retry_with_backoff, time_windows};
pub use view::finished_view_keeps_agent_and_start;

/// Uprząż testu: scheduler, skrypty wykonawczyń, czas, restart.
#[async_trait]
pub trait Harness: Send + Sync {
    /// Typ schedulera.
    type S: Scheduler;
    /// Scheduler.
    fn scheduler(&self) -> &Self::S;
    /// Skrypt wykonawczyni dla zadania (przed zgłoszeniem).
    fn script(&self, task: &TaskId, script: Script);
    /// Przesuwa czas o `ms` (kroki wykonawczyń, terminy, ponowienia).
    async fn advance(&self, ms: u64);
    /// Steering widziany przez wykonawczynię zadania.
    fn seen_steering(&self, task: &TaskId) -> Vec<(u32, SteerEnvelope)>;
    /// Wszystkie wysłania (w kolejności).
    fn dispatches(&self) -> Vec<Dispatch>;
    /// Zasoby trzymane teraz przez zadania.
    fn held(&self) -> BTreeMap<TaskId, Vec<Resource>>;
    /// Bieżący czas (ms).
    fn now_ms(&self) -> u64;
    /// Restart procesu: wykonawczynie giną, nowy scheduler ze stanu z magazynu.
    async fn restart(&mut self);
}

/// Jednokrotne odpytanie przyszłości (bez czekania).
pub async fn poll_once<F: Future + Unpin>(fut: &mut F) -> Option<F::Output> {
    std::future::poll_fn(|cx| match Pin::new(&mut *fut).poll(cx) {
        Poll::Ready(v) => Poll::Ready(Some(v)),
        Poll::Pending => Poll::Ready(None),
    })
    .await
}

/// Zadanie użytkownika dla dowolnej agentki.
pub fn task(id: &str, class: TaskClass) -> TaskSpec {
    TaskSpec::new(id, id, Assignee::AnyAgent, class, TaskOrigin::User)
}

/// Stan zadania (panika, gdy nieznane).
pub fn state_of<H: Harness>(h: &H, id: &str) -> TaskState {
    h.scheduler()
        .task(&TaskId::new(id))
        .unwrap_or_else(|| panic!("brak zadania {id}"))
        .state
}

/// Zakończenie zadania (panika, gdy jeszcze trwa).
pub fn done<H: Harness>(h: &H, id: &str) -> Termination {
    match state_of(h, id) {
        TaskState::Done { termination } => termination,
        other => panic!("zadanie {id} jeszcze trwa: {other:?}"),
    }
}

/// Czy zadanie zakończyło się sukcesem.
pub fn succeeded<H: Harness>(h: &H, id: &str) -> bool {
    done(h, id).is_success()
}

/// Sprawdza wyłączność: żaden zasób nie jest trzymany przez dwa zadania naraz.
pub fn assert_exclusive<H: Harness>(h: &H) {
    let mut owners: BTreeMap<Resource, TaskId> = BTreeMap::new();
    for (task, resources) in h.held() {
        for r in resources {
            if let Some(other) = owners.insert(r.clone(), task.clone()) {
                panic!("zasób {r} trzymany naraz przez {other} i {task}");
            }
        }
    }
}

/// Przesuwa czas krokami po `step` ms, sprawdzając wyłączność po każdym.
pub async fn advance_checked<H: Harness>(h: &H, total: u64, step: u64) {
    let mut left = total;
    while left > 0 {
        let d = left.min(step);
        h.advance(d).await;
        assert_exclusive(h);
        left -= d;
    }
}

/// Cały zestaw; `factory` daje świeżą uprząż.
pub async fn run_all<H, F, Fut>(factory: F)
where
    H: Harness,
    F: Fn() -> Fut,
    Fut: Future<Output = H>,
{
    dag_with_conditions_and_inputs(&factory().await).await;
    cancel_subtree(&factory().await).await;
    exclusive_resources_in_parallel(&factory().await).await;
    priorities_and_preemption(&factory().await).await;
    voice_first(&factory().await).await;
    time_windows(&factory().await).await;
    retry_with_backoff(&factory().await).await;
    budgets(&factory().await).await;
    steering_within_one_step(&factory().await).await;
    restart_resumes(&mut factory().await).await;
    kill_switch_cancels_everything(&factory().await).await;
    finished_view_keeps_agent_and_start(&factory().await).await;
}
