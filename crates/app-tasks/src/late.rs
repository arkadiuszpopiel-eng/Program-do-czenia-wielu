//! Wykonawczyni wiązana po złożeniu rdzenia: scheduler startuje z modułami (głos potrzebuje
//! tablicy blokad od razu), a wykonawczyni zadań potrzebuje rdzenia aplikacji (Replay, sesje).
//! Zadanie wysłane przed związaniem czeka na nie (≤ [`BIND_WAIT`]), potem kończy się błędem
//! ponawialnym — scheduler ponowi je z odstępem.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use scheduler_contract::{Dispatch, StepGate, TaskExecutor, WorkerResult};
use tokio::sync::Notify;

/// Najdłuższe czekanie na związanie wykonawczyni.
pub const BIND_WAIT: Duration = Duration::from_secs(30);

/// Wykonawczyni z późnym wiązaniem.
#[derive(Default)]
pub struct LateExecutor {
    inner: OnceLock<Arc<dyn TaskExecutor>>,
    bound: Notify,
}

impl LateExecutor {
    /// Nowa (niezwiązana).
    pub fn new() -> Self {
        Self::default()
    }

    /// Wiąże wykonawczynię (tylko raz; kolejne wywołania są ignorowane).
    pub fn bind(&self, executor: Arc<dyn TaskExecutor>) {
        if self.inner.set(executor).is_ok() {
            self.bound.notify_waiters();
        }
    }

    async fn get(&self) -> Option<Arc<dyn TaskExecutor>> {
        if let Some(e) = self.inner.get() {
            return Some(e.clone());
        }
        let notified = self.bound.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if let Some(e) = self.inner.get() {
            return Some(e.clone());
        }
        let _ = tokio::time::timeout(BIND_WAIT, notified).await;
        self.inner.get().cloned()
    }
}

#[async_trait]
impl TaskExecutor for LateExecutor {
    async fn execute(&self, dispatch: Dispatch, gate: Arc<dyn StepGate>) -> WorkerResult {
        match self.get().await {
            Some(executor) => executor.execute(dispatch, gate).await,
            None => WorkerResult::Failed {
                error: "wykonawczyni zadań jeszcze nie gotowa".into(),
                retryable: true,
            },
        }
    }
}
