//! Kill-switch aplikacji przy Brokerze poza procesem (wydzielony z `remote.rs` — limit rozmiaru pliku).

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, Level};
use platform_contract::{ProcessHandle, ProcessPort};
use safety_broker_contract::event_kind;
use safety_broker_contract::ipc::{Request, Response};
use serde_json::json;
use watchdog_contract::{
    EVENT_AUDIO_SILENCE, JobRecord, JobRegistry, JobTable, KillReason, KillReport, KillSwitch,
    ProcessRole,
};

use crate::link::BrokerLink;
use crate::remote::KILL_TIMEOUT;
/// Kill-switch po stronie aplikacji przy Brokerze poza procesem: drzewa procesów narzędzi
/// (uchwyty Job Objects należą do procesu aplikacji — zabija je ten sam port, który je
/// uruchomił), cisza audio na magistrali, potem `KillAll` w Brokerze (tokeny, prośby, plany).
pub struct RemoteKill {
    link: Arc<BrokerLink>,
    jobs: JobTable,
    processes: Arc<dyn ProcessPort>,
    bus: Option<Arc<dyn EventBus>>,
}

impl std::fmt::Debug for RemoteKill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteKill")
            .field("jobs", &self.jobs)
            .finish_non_exhaustive()
    }
}

impl RemoteKill {
    /// Kill-switch nad łączem, portem procesów narzędzi i magistralą (cisza audio).
    pub fn new(
        link: Arc<BrokerLink>,
        processes: Arc<dyn ProcessPort>,
        bus: Option<Arc<dyn EventBus>>,
    ) -> Self {
        Self {
            link,
            jobs: JobTable::default(),
            processes,
            bus,
        }
    }
}

#[async_trait]
impl KillSwitch for RemoteKill {
    async fn kill_all(&self, reason: KillReason) -> KillReport {
        let started = Instant::now();
        let (jobs_killed, jobs_failed) = self.jobs.kill_all(self.processes.as_ref());
        let audio_silenced = match &self.bus {
            Some(bus) => {
                let ev = Event::new(
                    event_kind(EVENT_AUDIO_SILENCE),
                    Level::Warn,
                    json!({ "reason": reason }),
                );
                bus.publish(ev).await.is_ok()
            }
            None => false,
        };
        let request = Request::KillAll {
            reason: reason.clone(),
        };
        let (tokens_revoked, audited) = match self
            .link
            .call_async_within(request, KILL_TIMEOUT)
            .await
        {
            Ok(Response::Killed(r)) => (r.tokens_revoked, r.audited),
            Ok(other) => {
                tracing::error!(odpowiedz = ?other, "kill-switch: nieoczekiwana odpowiedź Sędziego Cieni");
                (0, false)
            }
            Err(e) => {
                tracing::error!(error = %e, "kill-switch: Sędzia Cieni niedostępny (tokeny wygasną same)");
                (0, false)
            }
        };
        KillReport {
            reason,
            tokens_revoked,
            jobs_killed,
            jobs_failed,
            audio_silenced,
            audited,
            latency_us: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        }
    }
}

impl JobRegistry for RemoteKill {
    fn register_job(&self, job: ProcessHandle, owner: ProcessRole, label: &str) {
        self.jobs.register_job(job, owner, label);
    }

    fn unregister_job(&self, job: ProcessHandle) -> bool {
        self.jobs.unregister_job(job)
    }

    fn jobs(&self) -> Vec<JobRecord> {
        self.jobs.jobs()
    }
}
