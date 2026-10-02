//! Zadania, wyzwalacze i Marszałek w aplikacji (kategoria `app-*`, wydzielona z `app-core` —
//! limit rozmiaru crate'a):
//! - [`modules`] — `scheduler-impl` (zastępuje `scheduler-lite-impl`: ta sama tablica blokad dla
//!   mowy i zadań; stan w `%LOCALAPPDATA%\Alfa\scheduler`, budżet tła z `cost-meter`),
//!   `triggers-impl` (obserwacja katalogów: port platformy przez [`watch`], bez niego [`NoFileWatch`]),
//!   `marshal-impl` (tłumacz [`LlmTranslator`] przez Router);
//! - [`LateExecutor`] / [`AppExecutor`] — wykonawczyni zadań: agentka przez `RuntimeExecutor`
//!   z hakiem `StepGate::boundary`, most CLI przez `agent-backends` (pochodzenie zadania →
//!   zgoda), Replay przez port [`TaskHost`];
//! - [`TasksApp`] — komendy `tasks_*`, `triggers_*`, `marshal_*` (DTO `app-api`);
//! - [`bridge`] — zdarzenia → UI, DND z `voice-wake`, warunki systemowe, obsada schedulera.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod app;
pub mod bridge;
pub mod bridge_view;
mod delegate;
mod exec_agent;
mod exec_bridge;
mod executor;
mod host;
mod late;
pub mod map;
mod rules;
mod sink;
pub mod text;
mod translator;
pub mod watch;

use std::path::Path;
use std::sync::Arc;

use app_api::AppError;
use cost_meter_contract::CostMeter;
use marshal_contract::RuleTranslator;
use marshal_impl::FileMarshalStore;
use scheduler_contract::{Scheduler, TaskExecutor};
use scheduler_impl::{CostMeterBudget, FileSnapshotStore};
use triggers_impl::FileTriggerStore;

pub use app::{TasksApp, TasksParts};
pub use bridge::{RosterCtl, spawn_bus_bridge, spawn_conditions};
pub use bridge_view::{BridgeProjector, bridge_name};
pub use delegate::{Delegated, is_unverified};
pub use exec_agent::{TaskRuntimes, goal_of, origin_of};
pub use exec_bridge::UNVERIFIED_NOTE;
pub use executor::AppExecutor;
pub use host::{AgentKit, ExecDeps, TaskHost};
pub use late::{BIND_WAIT, LateExecutor};
pub use marshal_impl::MarshalModule;
pub use marshal_impl::NoTranslator;
pub use scheduler_impl::SchedulerModule;
pub use sink::{BridgeRuns, BrokerSink, NoBrokerSink};
pub use translator::{LlmTranslator, parse_drafts};
pub use triggers_impl::{NoFileWatch, TriggersModule};

/// Manifesty modułów składanych przez ten crate (rejestr `core-registry` w `app-core`).
pub const MODULES: &[(&str, &str)] = &[
    ("scheduler", scheduler_impl::MODULE_TOML),
    ("triggers", triggers_impl::MODULE_TOML),
    ("marshal", marshal_impl::MODULE_TOML),
];

/// Moduły (budowane w kolejności rejestru; start robi `app-core`).
pub mod modules {
    use super::*;

    /// Scheduler: stan `<dir>/scheduler.json` (restart = wznowienie), budżet tła `cost-meter`.
    pub fn scheduler(
        dir: &Path,
        executor: Arc<dyn TaskExecutor>,
        meter: Arc<dyn CostMeter>,
    ) -> Result<SchedulerModule, AppError> {
        SchedulerModule::new(
            executor,
            Arc::new(FileSnapshotStore::new(dir.join("scheduler.json"))),
            Arc::new(CostMeterBudget::new(meter)),
        )
        .map_err(|e| AppError::internal(format!("scheduler: {e}")))
    }

    /// Wyzwalacze: stan `<dir>/triggers.json`; obserwacja katalogów — port platformy
    /// (`None` — bez obserwacji: wyzwalacze plikowe tylko przez `file_created`).
    pub fn triggers(
        dir: &Path,
        scheduler: Arc<dyn Scheduler>,
        watch: Option<Arc<dyn platform_contract::DirWatchPort>>,
    ) -> Result<TriggersModule, AppError> {
        let files: Arc<dyn triggers_impl::FileWatchPort> = match watch {
            Some(port) => Arc::new(crate::watch::PlatformFileWatch::new(port)),
            None => Arc::new(NoFileWatch),
        };
        TriggersModule::new(
            scheduler,
            Arc::new(FileTriggerStore::new(dir.join("triggers.json"))),
            files,
        )
        .map_err(|e| AppError::internal(format!("triggers: {e}")))
    }

    /// Marszałek: księga reguł `<dir>/marshal.json`.
    pub fn marshal(
        dir: &Path,
        translator: Arc<dyn RuleTranslator>,
    ) -> Result<MarshalModule, AppError> {
        MarshalModule::new(
            translator,
            Arc::new(FileMarshalStore::new(dir.join("marshal.json"))),
        )
        .map_err(|e| AppError::internal(format!("marshal: {e}")))
    }
}
