//! Kontrakt watchdoga (docs/modules/watchdog/SPEC.md, PLAN §8.6, §12.2).
//!
//! Watchdog jest osobnym, minimalnym procesem: heartbeat procesów Alfy, restart z limitem,
//! safe-mode po pętli awarii, rollback konfiguracji/wersji do „ostatniej dobrej” (z cooldownem),
//! rejestr Job Objects i kill-switch (< 200 ms, bez zatwierdzeń, niezależnie od Brokera).
//! Typy kill-switcha ([`KillReason`], [`KillReport`], [`KillSwitch`], [`JobTable`]) i zegar
//! ([`Clock`], [`ManualClock`]) są wspólne z `safety-broker`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod clock;
mod kill;
mod watch;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use clock::{Clock, ManualClock, SystemClock};
pub use kill::{
    EVENT_AUDIO_SILENCE, EVENT_KILL_SWITCH, JobFailure, JobRecord, JobRegistry, JobTable,
    KillReason, KillReport, KillSwitch, ProcessRole,
};
pub use watch::{
    ConfigHistory, EVENT_CRASH_LOOP, EVENT_HEARTBEAT_MISSED, EVENT_RESTART, EVENT_ROLLBACK,
    EVENT_SAFE_MODE_ENTERED, EVENT_SAFE_MODE_LEFT, Health, Heartbeat, ManualConfirmation,
    SafeModeState, Supervisor, UpdaterSignal, WatchAction, WatchPolicy, Watchdog, WatchdogError,
};

use core_bus_contract::EventKind;

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_contract::{
        PlatformError, ProcessHandle, ProcessPort, ProcessSpec, ProcessStatus,
    };

    struct Port;

    impl ProcessPort for Port {
        fn spawn(&self, _: ProcessSpec) -> Result<ProcessHandle, PlatformError> {
            Err(PlatformError::Unsupported("test".into()))
        }
        fn kill_tree(&self, h: ProcessHandle) -> Result<(), PlatformError> {
            if h.0 == 2 {
                Err(PlatformError::UnknownResource("proces 2".into()))
            } else {
                Ok(())
            }
        }
        fn status(&self, _: ProcessHandle) -> Result<ProcessStatus, PlatformError> {
            Ok(ProcessStatus::Running)
        }
        fn foreground_is_elevated(&self) -> bool {
            false
        }
    }

    #[test]
    fn job_table_kills_all_and_reports_failures() {
        let t = JobTable::default();
        for i in 1..=3 {
            t.register_job(ProcessHandle(i), ProcessRole::Tool(format!("t{i}")), "x");
        }
        assert!(t.unregister_job(ProcessHandle(3)));
        assert!(!t.unregister_job(ProcessHandle(3)));
        assert_eq!(t.jobs().len(), 2);
        let (killed, failed) = t.kill_all(&Port);
        assert_eq!(killed, 1);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].job, 2);
        assert!(t.jobs().is_empty());
    }

    #[test]
    fn serde_and_display() {
        let r = KillReason::Broker("naruszenie".into());
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"source": "broker", "detail": "naruszenie"})
        );
        assert_eq!(KillReason::Hotkey.to_string(), "skrót globalny");
        assert_eq!(
            ProcessRole::Sidecar("voice-stt".into()).to_string(),
            "sidecar:voice-stt"
        );
        let a = WatchAction::Restart {
            role: ProcessRole::Core,
            attempt: 1,
        };
        let back: WatchAction = serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
        assert_eq!(back, a);
        assert_eq!(WatchPolicy::default().max_restarts, 3);
    }
}
