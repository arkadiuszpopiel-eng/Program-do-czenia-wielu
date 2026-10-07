//! Kill-switch Brokera: unieważnia wszystkie tokeny (nowy klucz bez okna łaski), wygasza
//! prośby, „zawsze zezwalaj” i plany, zabija zarejestrowane drzewa procesów, wysyła ciszę audio.
//! Nie wymaga zatwierdzeń; błąd Audytu nie blokuje kill-switcha.

use std::time::Instant;

use async_trait::async_trait;
use core_bus_contract::{Event, Level};
use platform_contract::ProcessHandle;
use safety_broker_contract::EVENT_KILL_SWITCH;
use serde_json::json;
use watchdog_contract::{
    EVENT_AUDIO_SILENCE, JobRecord, JobRegistry, KillReason, KillReport, KillSwitch, ProcessRole,
    event_kind,
};

use crate::engine::BrokerEngine;
use crate::state::PendStatus;

impl BrokerEngine {
    fn revoke_everything(&self) -> u64 {
        let mut st = self.lock();
        let revoked = st.tokens.len() as u64;
        st.tokens.clear();
        // Nowy klucz bez okna łaski: nawet nieznany rejestrowi token nie przejdzie MAC.
        let _ = st.keys.wipe();
        st.grants.clear();
        st.plans.clear();
        for p in st.approvals.values_mut() {
            if matches!(
                p.status,
                PendStatus::Pending | PendStatus::Approved(Some(_))
            ) {
                p.status = PendStatus::Expired;
            }
        }
        revoked
    }
}

#[async_trait]
impl KillSwitch for BrokerEngine {
    async fn kill_all(&self, reason: KillReason) -> KillReport {
        let started = Instant::now();
        let tokens_revoked = self.revoke_everything();
        let (jobs_killed, jobs_failed) = self.jobs.kill_all(self.processes.as_ref());
        let audio_silenced = match &self.bus {
            Some(bus) => {
                let payload = json!({ "reason": reason });
                let ev = Event::new(event_kind(EVENT_AUDIO_SILENCE), Level::Warn, payload);
                bus.publish(ev).await.is_ok()
            }
            None => false,
        };
        let latency_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let payload = json!({
            "reason": reason, "tokens_revoked": tokens_revoked, "jobs_killed": jobs_killed,
            "jobs_failed": jobs_failed, "latency_us": latency_us,
        });
        let audited = self.audit(EVENT_KILL_SWITCH, None, payload).is_ok();
        KillReport {
            reason,
            tokens_revoked,
            jobs_killed,
            jobs_failed,
            audio_silenced,
            audited,
            latency_us,
        }
    }
}

impl JobRegistry for BrokerEngine {
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
