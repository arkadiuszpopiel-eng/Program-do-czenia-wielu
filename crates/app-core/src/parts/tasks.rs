//! Zadania, wyzwalacze, Marszałek i mosty CLI w kompozycji: `scheduler-impl` zastępuje
//! `scheduler-lite-impl` (ta sama tablica blokad dla głosu i zadań), wykonawczyni zadań wiązana
//! po złożeniu rdzenia (Replay przez `TaskHost`), `triggers-impl` (obserwacja katalogów — port
//! platformy `DirWatchPort` + pompa nowych plików), `marshal-impl` (tłumacz przez Router), mosty `agent-backends` (kanał
//! zatwierdzeń = Broker agentek) i serwer MCP Alfy startowany na żądanie mostu.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use agent_backends_contract::{AgentBackend, ApprovalSink};
use app_bridges::{BridgeHandle, BridgesApp, BridgesParts, LazyMcpHost, proxy_program};
use app_modules::broker::path_env_for;
use app_tasks::{
    AgentKit, AppExecutor, BridgeRuns, BrokerSink, ExecDeps, LateExecutor, LlmTranslator,
    MarshalModule, NoBrokerSink, RosterCtl, SchedulerModule, TaskHost, TasksApp, TasksParts,
    TriggersModule,
};
use core_bus_contract::{EventBus, SessionId};
use personas_contract::Personas;
use safety_broker_contract::Broker;
use tools_common_contract::BrokerGate;

use super::agents::AgentStack;
use super::memory::setting;
use super::{Built, Kernel, need};
use crate::compose::{HealthSlot, started};
use crate::core::AppCore;
use crate::error::AppError;
use crate::options::{AppOptions, AppPaths};
use crate::ports::{BrainPort, ShellPort};

/// Limit równoległych zadań agentek (Ustawienia → Agentki).
pub(crate) const MAX_PARALLEL: &str = "scheduler.max_parallel";
const DEFAULT_PARALLEL: u32 = 4;
/// Obsada schedulera: obsada domyślna (sesja bez własnej obsady).
const ROSTER_SESSION: &str = "system-tasks";

/// Moduły zadań zbudowane w kolejności rejestru.
#[derive(Default)]
pub(crate) struct TaskParts {
    late: Arc<LateExecutor>,
    translator: Arc<LlmTranslator>,
    bridges_denied: Arc<AtomicBool>,
    runs: Arc<BridgeRuns>,
    pub scheduler: Option<Arc<SchedulerModule>>,
    triggers: Option<Arc<TriggersModule>>,
    marshal: Option<Arc<MarshalModule>>,
    roster: Option<Arc<RosterCtl>>,
}

/// Zależności złożenia (po portach).
pub(crate) struct StackDeps<'a> {
    pub options: &'a AppOptions,
    pub paths: &'a AppPaths,
    pub kernel: &'a Kernel,
    pub bus: &'a Arc<dyn EventBus>,
    pub shell: &'a Arc<dyn ShellPort>,
    pub brain: &'a Arc<dyn BrainPort>,
    pub agents: Option<&'a AgentStack>,
    pub translator: bool,
}

/// Zadania i mosty dla `Inner` + wiązanie wykonawczyni po złożeniu rdzenia.
pub(crate) struct TaskStack {
    pub tasks: Arc<TasksApp>,
    pub bridges: Arc<BridgesApp>,
    pub binder: TaskBinder,
}

/// Wiąże wykonawczynię zadań z rdzeniem (Replay, sesje) — po zbudowaniu `AppCore`.
pub(crate) struct TaskBinder {
    late: Arc<LateExecutor>,
    runs: Arc<BridgeRuns>,
    bridges_denied: Arc<AtomicBool>,
    handle: Arc<dyn AgentBackend>,
    brain: Arc<dyn BrainPort>,
    personas: Arc<dyn Personas>,
    kit: Option<AgentKit>,
    bus: Arc<dyn EventBus>,
}

impl TaskBinder {
    /// Związanie (rdzeń trzymany słabo — bez cyklu `Inner` → scheduler → rdzeń).
    pub fn bind(self, core: &AppCore) {
        let host: Arc<dyn TaskHost> = Arc::new(crate::host::CoreHost::new(core));
        self.runs.bind(host.clone());
        let deps = ExecDeps {
            host,
            brain: self.brain,
            personas: self.personas,
            kit: self.kit,
            bridges: Some(self.handle),
            runs: self.runs,
            bus: self.bus,
        };
        self.late
            .bind(Arc::new(AppExecutor::new(deps, self.bridges_denied)));
    }
}

impl Built {
    /// Moduły zadań i mostów (`false` — identyfikator spoza tej grupy).
    pub(super) async fn build_tasks(
        &mut self,
        id: &str,
        paths: &AppPaths,
        kernel: &Kernel,
        bus: &Arc<dyn EventBus>,
        slot: Option<&HealthSlot>,
    ) -> Result<bool, AppError> {
        let t = &mut self.tasks;
        match id {
            "scheduler" => {
                let personas = need(&self.personas, "personas")?;
                let max = setting(kernel, MAX_PARALLEL)
                    .await
                    .and_then(|v| v.as_u64())
                    .map_or(DEFAULT_PARALLEL, |n| {
                        u32::try_from(n.clamp(1, 16)).unwrap_or(1)
                    });
                let roster = Arc::new(RosterCtl::new(
                    personas.cast(&SessionId::new(ROSTER_SESSION)),
                    max,
                ));
                let module = app_tasks::modules::scheduler(
                    &paths.scheduler(),
                    t.late.clone(),
                    need(&self.costs, "cost-meter")?,
                )?;
                roster.apply(&module);
                t.scheduler = Some(started(module, bus, slot).await?);
                t.roster = Some(roster);
            }
            "triggers" => {
                let scheduler = need(&t.scheduler, "scheduler")?;
                let watch = kernel.dir_watch.clone();
                let module =
                    app_tasks::modules::triggers(&paths.scheduler(), scheduler, watch.clone())?;
                let module = started(module, bus, slot).await?;
                if let Some(port) = watch {
                    app_tasks::watch::spawn_pump(port, Arc::downgrade(&module));
                }
                t.triggers = Some(module);
            }
            "marshal" => {
                let module = app_tasks::modules::marshal(&paths.scheduler(), t.translator.clone())?;
                t.marshal = Some(started(module, bus, slot).await?);
            }
            // Backend mostów i serwer MCP powstają leniwie (pierwsze zadanie mostu).
            "agent-backends" | "mcp" => super::extra::healthy(slot),
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Zadania, wyzwalacze, reguły i mosty aplikacji.
    pub(super) fn task_stack(&self, d: StackDeps<'_>) -> Result<TaskStack, AppError> {
        let t = &self.tasks;
        let scheduler = need(&t.scheduler, "scheduler")?;
        let roster = t
            .roster
            .clone()
            .ok_or_else(|| AppError::internal("brak obsady schedulera"))?;
        t.translator.bind(d.brain.clone());
        let kit = d.agents.map(|a| AgentKit {
            tools: a.tools.clone(),
            tickets: a.tickets.clone(),
            launch: a.launch.clone(),
        });
        let worktrees = d.paths.user_root.join("Mosty");
        let sink: Arc<dyn ApprovalSink> = match &kit {
            Some(k) => Arc::new(BrokerSink {
                gate: BrokerGate::new(k.tickets.clone() as Arc<dyn Broker>),
                runs: t.runs.clone(),
                worktrees: worktrees.to_string_lossy().into_owned(),
                env: path_env_for(&d.paths.user_root).1,
                timeout: d
                    .options
                    .approval_timeout
                    .unwrap_or(Duration::from_secs(300)),
            }),
            None => Arc::new(NoBrokerSink),
        };
        let platform = Arc::new(platform_windows_impl::WindowsPlatform::default());
        let clipboard: Arc<dyn platform_contract::ClipboardPort> = match &d.options.clipboard {
            Some(c) => c.clone(),
            None => platform.clone(),
        };
        let mcp = LazyMcpHost::new(
            proxy_program(),
            mcp_impl::PlatformPorts {
                clipboard,
                windows: platform,
            },
        );
        let probe = d.options.cli_probe.clone().unwrap_or_else(|| {
            Arc::new(accounts_hub_impl::SystemCliProbe::from_env())
                as Arc<dyn accounts_hub_contract::CliProbe>
        });
        let bridges = Arc::new(BridgesApp::new(BridgesParts {
            compliance: need(&self.compliance, "compliance")?,
            config: d.kernel.config.clone(),
            probe,
            shell: d.shell.clone(),
            sink: sink.clone(),
            mcp: Arc::new(mcp),
            runtime_dir: d.paths.bridges(),
            worktrees,
            home: d
                .paths
                .user_root
                .parent()
                .map_or_else(|| d.paths.user_root.clone(), std::path::Path::to_path_buf),
            bus: d.bus.clone(),
            backend: d.options.bridges.as_ref().map(|f| f(sink)),
        }));
        let tasks = Arc::new(TasksApp::new(TasksParts {
            scheduler: scheduler.clone(),
            triggers: t.triggers.clone(),
            marshal: t.marshal.clone(),
            translator: d.translator,
            watch: false,
            bridges_denied: t.bridges_denied.clone(),
            roster,
            events: d.kernel.events.clone(),
        }));
        tasks.apply_policy();
        let binder = TaskBinder {
            late: t.late.clone(),
            runs: t.runs.clone(),
            bridges_denied: t.bridges_denied.clone(),
            handle: Arc::new(BridgeHandle::new(bridges.clone())),
            brain: d.brain.clone(),
            personas: need(&self.personas, "personas")?,
            kit,
            bus: d.bus.clone(),
        };
        Ok(TaskStack {
            tasks,
            bridges,
            binder,
        })
    }
}
