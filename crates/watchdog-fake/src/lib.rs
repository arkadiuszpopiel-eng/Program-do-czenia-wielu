//! Atrapa watchdoga (docs/modules/watchdog/SPEC.md, sekcja „Fake”) do testów `ui-quick`,
//! `safety-broker` i modułów z sidecarami: rejestruje heartbeaty, zgłoszenia awarii i wywołania
//! kill-switcha (jako zdarzenia — nic nie jest zabijane), safe-mode sterowany z testu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use platform_contract::ProcessHandle;
use watchdog_contract::{
    Health, Heartbeat, JobRecord, JobRegistry, JobTable, KillReason, KillReport, KillSwitch,
    ManualConfirmation, ProcessRole, SafeModeState, WatchAction, Watchdog, WatchdogError,
};

#[derive(Debug, Default)]
struct State {
    watched: BTreeMap<ProcessRole, bool>,
    heartbeats: Vec<Heartbeat>,
    crashes: Vec<(ProcessRole, String)>,
    kills: Vec<KillReason>,
    safe_mode: Option<SafeModeState>,
    log: Vec<WatchAction>,
    scripted_tick: Vec<WatchAction>,
}

/// Deterministyczna atrapa watchdoga.
#[derive(Debug, Default)]
pub struct FakeWatchdog {
    jobs: JobTable,
    state: Mutex<State>,
}

impl FakeWatchdog {
    /// Nowa atrapa.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Odebrane heartbeaty.
    pub fn heartbeats(&self) -> Vec<Heartbeat> {
        self.lock().heartbeats.clone()
    }

    /// Zgłoszone awarie (także `Health::Failing`).
    pub fn crashes(&self) -> Vec<(ProcessRole, String)> {
        self.lock().crashes.clone()
    }

    /// Wywołania kill-switcha.
    pub fn kills(&self) -> Vec<KillReason> {
        self.lock().kills.clone()
    }

    /// Wymusza safe-mode (test stanów UI §14.4).
    pub fn force_safe_mode(&self, reason: &str) {
        let mut st = self.lock();
        st.safe_mode = Some(SafeModeState {
            since_ms: 0,
            reason: reason.to_owned(),
        });
        st.log.push(WatchAction::EnterSafeMode {
            reason: reason.to_owned(),
        });
    }

    /// Akcje zwrócone przez następny `tick`.
    pub fn script_tick(&self, actions: Vec<WatchAction>) {
        self.lock().scripted_tick = actions;
    }
}

#[async_trait]
impl KillSwitch for FakeWatchdog {
    async fn kill_all(&self, reason: KillReason) -> KillReport {
        let jobs = self.jobs.jobs();
        for j in &jobs {
            self.jobs.unregister_job(j.job);
        }
        self.lock().kills.push(reason.clone());
        KillReport {
            reason,
            tokens_revoked: 0,
            jobs_killed: u32::try_from(jobs.len()).unwrap_or(u32::MAX),
            jobs_failed: Vec::new(),
            audio_silenced: true,
            audited: false,
            latency_us: 0,
        }
    }
}

impl JobRegistry for FakeWatchdog {
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

impl Watchdog for FakeWatchdog {
    fn watch(&self, role: ProcessRole, critical: bool) {
        self.lock().watched.insert(role, critical);
    }

    fn heartbeat(&self, hb: Heartbeat) -> Result<Vec<WatchAction>, WatchdogError> {
        let mut st = self.lock();
        if !st.watched.contains_key(&hb.from) {
            return Err(WatchdogError::NotWatched(hb.from));
        }
        if let Health::Failing(detail) = &hb.health {
            st.crashes.push((hb.from.clone(), detail.clone()));
        }
        st.heartbeats.push(hb);
        Ok(Vec::new())
    }

    fn report_crash(&self, role: &ProcessRole, detail: &str) -> Vec<WatchAction> {
        let mut st = self.lock();
        st.crashes.push((role.clone(), detail.to_owned()));
        let action = WatchAction::Restart {
            role: role.clone(),
            attempt: 1,
        };
        st.log.push(action.clone());
        vec![action]
    }

    fn tick(&self) -> Vec<WatchAction> {
        let mut st = self.lock();
        let actions = std::mem::take(&mut st.scripted_tick);
        st.log.extend(actions.iter().cloned());
        actions
    }

    fn mark_last_good(&self) {}

    fn safe_mode(&self) -> Option<SafeModeState> {
        self.lock().safe_mode.clone()
    }

    fn may_start(&self, role: &ProcessRole) -> bool {
        let st = self.lock();
        st.safe_mode.is_none()
            || matches!(
                role,
                ProcessRole::Core | ProcessRole::Broker | ProcessRole::BrokerUi
            )
            || st.watched.get(role).copied().unwrap_or(false)
    }

    fn leave_safe_mode(&self, _confirmation: ManualConfirmation) -> Result<(), WatchdogError> {
        let mut st = self.lock();
        st.safe_mode.take().ok_or(WatchdogError::NotInSafeMode)?;
        st.log.push(WatchAction::LeaveSafeMode);
        Ok(())
    }

    fn action_log(&self) -> Vec<WatchAction> {
        self.lock().log.clone()
    }
}
