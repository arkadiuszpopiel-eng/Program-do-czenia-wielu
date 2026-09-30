//! Współdzielony test kontraktowy (feature `contract-tests`) — ten sam dla `-impl` i `-fake`.
//! Czas steruje [`Harness::advance`] (fake: wirtualny zegar; impl: zatrzymany zegar tokio).

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

use async_trait::async_trait;
use personas_contract::PersonaId;

use crate::{Holder, LeaseRequest, Priority, Resource, SchedError, SchedulerLite};

mod advanced;
mod basic;

pub use advanced::{
    deadlock_fails_youngest, handoff_without_gap, kill_switch_and_cancel,
    user_speech_preempts_narration,
};
pub use basic::{
    grant_and_release_on_drop, priority_queue_is_fifo_within_priority, timeouts_follow_policy,
};

/// Uprząż testu: scheduler + sterowanie czasem.
#[async_trait]
pub trait Harness: Send + Sync {
    /// Typ schedulera.
    type S: SchedulerLite;
    /// Scheduler.
    fn scheduler(&self) -> &Self::S;
    /// Przesuwa czas o `ms` i pozwala sterownikowi obsłużyć terminy.
    async fn advance(&self, ms: u64);
}

/// Jednokrotne odpytanie przyszłości (bez czekania).
pub async fn poll_once<F: Future + Unpin>(fut: &mut F) -> Option<F::Output> {
    std::future::poll_fn(|cx| match Pin::new(&mut *fut).poll(cx) {
        Poll::Ready(v) => Poll::Ready(Some(v)),
        Poll::Pending => Poll::Ready(None),
    })
    .await
}

pub(crate) fn persona(name: &str) -> Holder {
    Holder::Persona(PersonaId::new(name))
}

pub(crate) fn req(resource: Resource, holder: Holder, priority: Priority, ms: u64) -> LeaseRequest {
    LeaseRequest::new(resource, holder, priority, Duration::from_millis(ms))
}

pub(crate) fn ok<T>(r: Option<Result<T, SchedError>>) -> T {
    match r {
        Some(Ok(v)) => v,
        Some(Err(e)) => panic!("błąd: {e}"),
        None => panic!("wynik jeszcze niegotowy"),
    }
}

/// Cały zestaw; `factory` daje świeżą uprząż.
pub async fn run_all<H, F, Fut>(factory: F)
where
    H: Harness,
    F: Fn() -> Fut,
    Fut: Future<Output = H>,
{
    grant_and_release_on_drop(&factory().await).await;
    priority_queue_is_fifo_within_priority(&factory().await).await;
    timeouts_follow_policy(&factory().await).await;
    user_speech_preempts_narration(&factory().await).await;
    handoff_without_gap(&factory().await).await;
    deadlock_fails_youngest(&factory().await).await;
    kill_switch_and_cancel(&factory().await).await;
}
