//! Nadzór: heartbeat, restart z limitem w oknie, safe-mode, rollback z cooldownem.

use core_bus_contract::Level;
use serde_json::json;
use watchdog_contract::{
    EVENT_CRASH_LOOP, EVENT_HEARTBEAT_MISSED, EVENT_RESTART, EVENT_ROLLBACK,
    EVENT_SAFE_MODE_ENTERED, EVENT_SAFE_MODE_LEFT, Health, Heartbeat, ManualConfirmation,
    ProcessRole, SafeModeState, WatchAction, Watchdog, WatchdogError,
};

use crate::{State, WatchdogService, Watched};

/// Role zawsze krytyczne (działają w safe-mode): jądro, Broker, Broker-UI.
fn always_critical(role: &ProcessRole) -> bool {
    matches!(
        role,
        ProcessRole::Core | ProcessRole::Broker | ProcessRole::BrokerUi
    )
}

impl WatchdogService {
    fn emit(st: &mut State, name: &str, level: Level, payload: serde_json::Value) {
        st.outbox.push((name.to_owned(), level, payload));
    }

    fn record(st: &mut State, actions: &[WatchAction]) {
        st.log.extend_from_slice(actions);
    }

    fn failure(&self, st: &mut State, role: &ProcessRole, detail: &str) -> Vec<WatchAction> {
        let now = self.now();
        let window = self.policy.window_ms;
        let max = usize::from(self.policy.max_restarts);
        let Some(w) = st.watched.get_mut(role) else {
            return Vec::new();
        };
        w.last_seen_ms = now;
        while w
            .restarts
            .front()
            .is_some_and(|t| now.saturating_sub(*t) >= window)
        {
            w.restarts.pop_front();
        }
        let critical = w.critical;
        let mut actions = Vec::new();
        if w.restarts.len() < max || critical {
            w.restarts.push_back(now);
            let attempt = u8::try_from(w.restarts.len()).unwrap_or(u8::MAX);
            let _ = self.ports.supervisor.restart(role);
            actions.push(WatchAction::Restart {
                role: role.clone(),
                attempt,
            });
            Self::emit(
                st,
                EVENT_RESTART,
                Level::Warn,
                json!({ "role": role, "attempt": attempt, "detail": detail }),
            );
            if attempt as usize <= max {
                Self::record(st, &actions);
                return actions;
            }
        }
        Self::emit(
            st,
            EVENT_CRASH_LOOP,
            Level::Error,
            json!({ "role": role, "detail": detail }),
        );
        if self.policy.safe_mode_after_crash_loop && st.safe_mode.is_none() {
            let reason = format!("pętla awarii: {role}");
            st.safe_mode = Some(SafeModeState {
                since_ms: now,
                reason: reason.clone(),
            });
            actions.push(WatchAction::EnterSafeMode {
                reason: reason.clone(),
            });
            Self::emit(
                st,
                EVENT_SAFE_MODE_ENTERED,
                Level::Audit,
                json!({ "reason": reason }),
            );
            let to_stop: Vec<ProcessRole> = st
                .watched
                .iter()
                .filter(|(_, w)| !w.critical && !w.stopped)
                .map(|(r, _)| r.clone())
                .collect();
            for r in to_stop {
                let _ = self.ports.supervisor.stop(&r);
                if let Some(w) = st.watched.get_mut(&r) {
                    w.stopped = true;
                }
                actions.push(WatchAction::StopForSafeMode { role: r });
            }
        }
        if self.policy.auto_rollback {
            actions.extend(self.rollback(st, now));
        }
        Self::record(st, &actions);
        actions
    }

    fn rollback(&self, st: &mut State, now: u64) -> Vec<WatchAction> {
        let skipped = |reason: &str| {
            vec![WatchAction::RollbackSkipped {
                reason: reason.to_owned(),
            }]
        };
        if st
            .last_rollback_ms
            .is_some_and(|t| now.saturating_sub(t) < self.policy.cooldown_ms)
        {
            return skipped("cooldown — rollback nie może iść w pętli");
        }
        let Some((good_rev, good_ver)) = st.last_good.clone() else {
            return skipped("brak konfiguracji/wersji oznaczonej jako ostatnia dobra");
        };
        let mut out = Vec::new();
        if let (Some(cfg), Some(rev)) = (&self.ports.config, good_rev)
            && cfg.current_revision().as_deref() != Some(rev.as_str())
            && cfg.rollback_to(&rev).is_ok()
        {
            out.push(WatchAction::RollbackConfig { revision: rev });
        }
        if let (Some(up), Some(ver)) = (&self.ports.updater, good_ver)
            && up.current_version() != ver
            && up.request_rollback(&ver).is_ok()
        {
            out.push(WatchAction::RollbackVersion { to: ver });
        }
        if out.is_empty() {
            return skipped("bieżąca konfiguracja i wersja są już ostatnimi dobrymi");
        }
        st.last_rollback_ms = Some(now);
        Self::emit(st, EVENT_ROLLBACK, Level::Audit, json!({ "actions": out }));
        out
    }
}

impl Watchdog for WatchdogService {
    fn watch(&self, role: ProcessRole, critical: bool) {
        let now = self.now();
        let critical = critical || always_critical(&role);
        self.lock().watched.insert(
            role,
            Watched {
                critical,
                last_seen_ms: now,
                restarts: Default::default(),
                stopped: false,
            },
        );
    }

    fn heartbeat(&self, hb: Heartbeat) -> Result<Vec<WatchAction>, WatchdogError> {
        let now = self.now();
        let mut st = self.lock();
        let in_safe_mode = st.safe_mode.is_some();
        let w = st
            .watched
            .get_mut(&hb.from)
            .ok_or_else(|| WatchdogError::NotWatched(hb.from.clone()))?;
        if in_safe_mode && !w.critical {
            w.stopped = true;
            let _ = self.ports.supervisor.stop(&hb.from);
            let actions = vec![WatchAction::StopForSafeMode { role: hb.from }];
            Self::record(&mut st, &actions);
            return Ok(actions);
        }
        w.last_seen_ms = now;
        match hb.health {
            Health::Failing(detail) => Ok(self.failure(&mut st, &hb.from, &detail)),
            Health::Ok | Health::Degraded(_) => Ok(Vec::new()),
        }
    }

    fn report_crash(&self, role: &ProcessRole, detail: &str) -> Vec<WatchAction> {
        let mut st = self.lock();
        self.failure(&mut st, role, detail)
    }

    fn tick(&self) -> Vec<WatchAction> {
        let now = self.now();
        let mut st = self.lock();
        let timeout = self.policy.heartbeat_timeout_ms;
        let missed: Vec<ProcessRole> = st
            .watched
            .iter()
            .filter(|(_, w)| !w.stopped && now.saturating_sub(w.last_seen_ms) >= timeout)
            .map(|(r, _)| r.clone())
            .collect();
        let mut actions = Vec::new();
        for role in missed {
            Self::emit(
                &mut st,
                EVENT_HEARTBEAT_MISSED,
                Level::Warn,
                serde_json::json!({ "role": role }),
            );
            actions.extend(self.failure(&mut st, &role, "brak heartbeatu"));
        }
        actions
    }

    fn mark_last_good(&self) {
        let rev = self
            .ports
            .config
            .as_ref()
            .and_then(|c| c.current_revision());
        let ver = self.ports.updater.as_ref().map(|u| u.current_version());
        self.lock().last_good = Some((rev, ver));
    }

    fn safe_mode(&self) -> Option<SafeModeState> {
        self.lock().safe_mode.clone()
    }

    fn may_start(&self, role: &ProcessRole) -> bool {
        let st = self.lock();
        st.safe_mode.is_none()
            || always_critical(role)
            || st.watched.get(role).is_some_and(|w| w.critical)
    }

    fn leave_safe_mode(&self, confirmation: ManualConfirmation) -> Result<(), WatchdogError> {
        let mut st = self.lock();
        if st.safe_mode.take().is_none() {
            return Err(WatchdogError::NotInSafeMode);
        }
        for w in st.watched.values_mut() {
            w.restarts.clear();
            w.stopped = false;
        }
        Self::record(&mut st, &[WatchAction::LeaveSafeMode]);
        let payload = serde_json::json!({ "surface": confirmation.surface });
        Self::emit(&mut st, EVENT_SAFE_MODE_LEFT, Level::Audit, payload);
        Ok(())
    }

    fn action_log(&self) -> Vec<WatchAction> {
        self.lock().log.clone()
    }
}
