//! Mikrofon jako zasób wyłączny w `scheduler-lite`: dzierżawa na czas słuchania (`Holder::User`,
//! priorytet `UserSpeech` — voice-first), zwalniana przy `ListenStop` / wyciszeniu.
//!
//! [`MicArbiter`] jest w kontrakcie (nie w `-impl`), bo składa go także runtime potoku głosu,
//! który zależy wyłącznie od kontraktów. [`lease_now`] — próba dzierżawy bez czekania z wątku
//! przetwarzania (bez `await` i bez blokowania).

use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use scheduler_lite_contract::{
    Holder, Lease, LeaseRequest, OnTimeout, Priority, Resource, SchedError, SchedulerLite,
};

use crate::WakeEvent;

/// Próba dzierżawy bez czekania: jedno odpytanie `acquire` (`max_wait` = 0 → rdzeń decyduje od
/// razu). Gdy wynik nie jest gotowy, żądanie jest porzucane (anulowane) i zwracany jest
/// `SchedError::Timeout` z `waited_ms = 0`.
pub fn lease_now(
    scheduler: &dyn SchedulerLite,
    resource: Resource,
    holder: Holder,
    priority: Priority,
) -> Result<Lease, SchedError> {
    let request = LeaseRequest::new(resource.clone(), holder, priority, Duration::ZERO);
    let mut fut = scheduler.acquire(request);
    let mut cx = Context::from_waker(Waker::noop());
    match fut.as_mut().poll(&mut cx) {
        Poll::Ready(result) => result,
        Poll::Pending => Err(SchedError::Timeout {
            resource,
            on_timeout: OnTimeout::Fail,
            waited_ms: 0,
        }),
    }
}

/// Pośrednik dzierżawy mikrofonu.
pub struct MicArbiter {
    scheduler: Arc<dyn SchedulerLite>,
    lease: Option<Lease>,
    max_wait: Duration,
}

impl std::fmt::Debug for MicArbiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicArbiter")
            .field("held", &self.lease.is_some())
            .finish_non_exhaustive()
    }
}

impl MicArbiter {
    /// Pośrednik; `max_wait` — ile czekać na mikrofon (zwykle 0: PTT ma działać od razu).
    pub fn new(scheduler: Arc<dyn SchedulerLite>, max_wait: Duration) -> Self {
        Self {
            scheduler,
            lease: None,
            max_wait,
        }
    }

    /// Czy mikrofon jest trzymany.
    pub fn held(&self) -> bool {
        self.lease.is_some()
    }

    /// Czy dzierżawa została odebrana (kill-switch) — mikrofon trzeba zamknąć.
    pub fn revoked(&self) -> bool {
        self.lease.as_ref().is_some_and(Lease::is_revoked)
    }

    /// Zwalnia mikrofon (np. po odebraniu dzierżawy).
    pub fn release(&mut self) {
        self.lease = None;
    }

    /// Reaguje na zdarzenia aktywacji: `ListenStart` → dzierżawa, `ListenStop` → zwolnienie.
    pub async fn apply(&mut self, events: &[WakeEvent]) -> Result<(), SchedError> {
        for e in events {
            match e {
                WakeEvent::ListenStart { .. } if self.lease.is_none() => {
                    let req = LeaseRequest::new(
                        Resource::Mic,
                        Holder::User,
                        Priority::UserSpeech,
                        self.max_wait,
                    );
                    self.lease = Some(self.scheduler.acquire(req).await?);
                }
                WakeEvent::ListenStop { .. } => self.lease = None,
                _ => {}
            }
        }
        Ok(())
    }

    /// Jak [`MicArbiter::apply`], ale bez czekania (wątek przetwarzania potoku): mikrofon
    /// przydzielony od razu albo błąd ([`lease_now`]).
    pub fn apply_now(&mut self, events: &[WakeEvent]) -> Result<(), SchedError> {
        for e in events {
            match e {
                WakeEvent::ListenStart { .. } if self.lease.is_none() => {
                    self.lease = Some(lease_now(
                        self.scheduler.as_ref(),
                        Resource::Mic,
                        Holder::User,
                        Priority::UserSpeech,
                    )?);
                }
                WakeEvent::ListenStop { .. } => self.lease = None,
                _ => {}
            }
        }
        Ok(())
    }
}
