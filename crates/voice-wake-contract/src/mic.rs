//! Mikrofon jako zasób wyłączny w `scheduler-lite`: dzierżawa na czas słuchania (`Holder::User`,
//! priorytet `UserSpeech` — voice-first), zwalniana przy `ListenStop` / wyciszeniu.

use std::sync::Arc;
use std::time::Duration;

use scheduler_lite_contract::{
    Holder, Lease, LeaseRequest, Priority, Resource, SchedError, SchedulerLite,
};
use voice_wake_contract::WakeEvent;

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
}
