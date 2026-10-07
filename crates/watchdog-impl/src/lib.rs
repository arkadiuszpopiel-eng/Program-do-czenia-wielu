//! Watchdog — logika (docs/modules/watchdog/SPEC.md, PLAN §8.6, §12.2). Binarka procesu,
//! hook skrótu `Ctrl+Shift+F12` i zasobnik to część 2; tu deterministyczny silnik sterowany
//! zegarem ([`watchdog_contract::Clock`]) i portami (`Supervisor`, `ConfigHistory`,
//! `UpdaterSignal`, `ProcessPort`). Zdarzenia z metod synchronicznych trafiają do kolejki
//! publikowanej przez [`WatchdogService::flush_events`] (pętla usługi po każdym `tick`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod daemon;
mod kill;
mod supervise;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::{Event, EventBus, Level};
use core_log_contract::AuditWriter;
use platform_contract::ProcessPort;
use watchdog_contract::{
    Clock, ConfigHistory, JobTable, KillSwitch, ProcessRole, SafeModeState, Supervisor,
    UpdaterSignal, WatchAction, WatchPolicy, event_kind,
};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

struct Watched {
    critical: bool,
    last_seen_ms: u64,
    restarts: VecDeque<u64>,
    stopped: bool,
}

struct State {
    watched: BTreeMap<ProcessRole, Watched>,
    safe_mode: Option<SafeModeState>,
    last_good: Option<(Option<String>, Option<String>)>,
    last_rollback_ms: Option<u64>,
    log: Vec<WatchAction>,
    outbox: Vec<(String, Level, serde_json::Value)>,
}

/// Porty watchdoga.
pub struct WatchdogPorts {
    /// Zegar.
    pub clock: Arc<dyn Clock>,
    /// Zabijanie drzew procesów (Job Objects).
    pub processes: Arc<dyn ProcessPort>,
    /// Uruchamianie/zatrzymywanie procesów.
    pub supervisor: Arc<dyn Supervisor>,
    /// Historia konfiguracji (`core-config`).
    pub config: Option<Arc<dyn ConfigHistory>>,
    /// Sygnał do `updater`.
    pub updater: Option<Arc<dyn UpdaterSignal>>,
    /// Magistrala (cisza audio, zdarzenia diagnostyczne).
    pub bus: Option<Arc<dyn EventBus>>,
    /// Audyt (przez Brokera) — best effort, nigdy nie blokuje kill-switcha.
    pub audit: Option<Arc<dyn AuditWriter>>,
    /// Inne kill-switche (Broker: unieważnienie tokenów) — wołane z limitem czasu.
    pub peers: Vec<Arc<dyn KillSwitch>>,
}

/// Silnik watchdoga.
pub struct WatchdogService {
    policy: WatchPolicy,
    ports: WatchdogPorts,
    jobs: JobTable,
    state: Mutex<State>,
}

impl WatchdogService {
    /// Nowy watchdog.
    pub fn new(policy: WatchPolicy, ports: WatchdogPorts) -> Self {
        Self {
            policy,
            ports,
            jobs: JobTable::default(),
            state: Mutex::new(State {
                watched: BTreeMap::new(),
                safe_mode: None,
                last_good: None,
                last_rollback_ms: None,
                log: Vec::new(),
                outbox: Vec::new(),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn now(&self) -> u64 {
        self.ports.clock.now_ms()
    }

    /// Publikuje zaległe zdarzenia (magistrala; safe-mode, rollback i kill — także Audyt).
    pub async fn flush_events(&self) -> usize {
        let outbox = std::mem::take(&mut self.lock().outbox);
        let n = outbox.len();
        for (name, level, payload) in outbox {
            let event = Event::new(event_kind(&name), level, payload);
            if level == Level::Audit
                && let Some(audit) = &self.ports.audit
            {
                let _ = audit.append_audit(&event).await;
            }
            if let Some(bus) = &self.ports.bus {
                let _ = bus.publish(event).await;
            }
        }
        n
    }
}
