//! Kill-switch watchdoga: cisza audio, zabicie wszystkich drzew procesów, przekazanie do
//! Brokera (unieważnienie tokenów) i Audyt — dwa ostatnie z limitem czasu, żeby brak lub
//! zawieszenie Brokera nigdy nie zablokowało kill-switcha.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use core_bus_contract::{Event, Level};
use platform_contract::ProcessHandle;
use serde_json::json;
use watchdog_contract::{
    EVENT_AUDIO_SILENCE, EVENT_KILL_SWITCH, JobRecord, JobRegistry, KillReason, KillReport,
    KillSwitch, ProcessRole, event_kind,
};

use crate::WatchdogService;

/// Limit czasu na peera (Broker) i na zapis Audytu.
const PEER_TIMEOUT: Duration = Duration::from_millis(100);
const AUDIT_TIMEOUT: Duration = Duration::from_millis(50);

#[async_trait]
impl KillSwitch for WatchdogService {
    async fn kill_all(&self, reason: KillReason) -> KillReport {
        let started = Instant::now();
        let audio_silenced = match &self.ports.bus {
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
        let (mut jobs_killed, mut jobs_failed) = self.jobs.kill_all(self.ports.processes.as_ref());
        let mut tokens_revoked = 0;
        for peer in &self.ports.peers {
            if let Ok(r) = tokio::time::timeout(PEER_TIMEOUT, peer.kill_all(reason.clone())).await {
                tokens_revoked += r.tokens_revoked;
                jobs_killed = jobs_killed.saturating_add(r.jobs_killed);
                jobs_failed.extend(r.jobs_failed);
            }
        }
        let latency_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let payload = json!({
            "reason": reason, "latency_us": latency_us, "jobs_killed": jobs_killed,
            "jobs_failed": jobs_failed, "tokens_revoked": tokens_revoked,
        });
        let event = Event::new(event_kind(EVENT_KILL_SWITCH), Level::Audit, payload);
        let audited = match &self.ports.audit {
            Some(a) => matches!(
                tokio::time::timeout(AUDIT_TIMEOUT, a.append_audit(&event)).await,
                Ok(Ok(_))
            ),
            None => false,
        };
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

impl JobRegistry for WatchdogService {
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
