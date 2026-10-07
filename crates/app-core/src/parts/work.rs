//! Computer use, terminal, umiejętności, Kreator agentów i „Zdrowie systemu" w kompozycji —
//! składane przez `app-gui`, `app-terminal`, `app-skills`, `app-health` po portach i zadaniach;
//! zdrowie ich modułów trafia do gniazd rejestru odłożonych przy budowie w kolejności startu.

use std::sync::Arc;

use app_gui::{GuiApp, GuiMonitor, GuiPorts};
use app_health::{HealthApp, HealthDeps};
use app_skills::{BuilderApp, SkillsApp, WorkDeps};
use app_terminal::{Programs, TerminalApp};
use core_registry_contract::HealthStatus;

use super::{Built, Kernel};
use crate::compose::HealthSlot;
use crate::options::{AppOptions, AppPaths};
use crate::ports::{BrokerPort, ShellPort, VoicePort};

/// Moduły tej części (zdrowie po złożeniu, nie w kolejności startu).
pub(crate) fn is_work(id: &str) -> bool {
    app_gui::MODULES
        .iter()
        .chain(app_terminal::MODULES)
        .chain(app_skills::MODULES)
        .chain(app_health::MODULES)
        .chain(app_plugins::MODULES)
        .any(|(m, _)| *m == id)
}

/// Komendy tej części.
pub(crate) struct WorkStack {
    pub gui: Arc<GuiApp>,
    pub terminal: Arc<TerminalApp>,
    pub skills: Arc<SkillsApp>,
    pub builder: Arc<BuilderApp>,
    pub health: Arc<HealthApp>,
    /// Aktualizacje, „O programie”, restart przez launcher (`app-updates`).
    pub updates: Arc<app_updates::UpdatesApp>,
    /// Wtyczki Wasm (`app-plugins`; te same, które dają narzędzia agentkom).
    pub plugins: Arc<app_plugins::PluginsApp>,
    /// Modele i silniki, embedder wyszukiwania (`app-models`).
    pub models: Arc<app_models::ModelsApp>,
    /// Załączniki, eksport rozmowy, kopie zapasowe (`app-files`).
    pub files: Arc<app_files::FilesApp>,
}

/// Porty GUI i panel „Ekran" (przed narzędziami agentek).
pub(crate) fn gui(
    options: &AppOptions,
    paths: &AppPaths,
    kernel: &Kernel,
) -> (GuiPorts, Arc<GuiMonitor>) {
    let ports = options
        .gui
        .clone()
        .unwrap_or_else(|| GuiPorts::system(&paths.local));
    let monitor = Arc::new(GuiMonitor::new(
        Some(kernel.events.clone()),
        ports.available,
    ));
    (ports, monitor)
}

/// Zależności złożenia.
pub(crate) struct WorkDepsIn<'a> {
    pub options: &'a AppOptions,
    pub paths: &'a AppPaths,
    pub kernel: &'a Kernel,
    pub registry: Arc<core_registry_impl::ModuleRegistry>,
    pub bus: &'a Arc<dyn core_bus_contract::EventBus>,
    pub monitor: Arc<GuiMonitor>,
    pub broker: Arc<dyn BrokerPort>,
    pub shell: &'a Arc<dyn ShellPort>,
    pub voice: Arc<dyn VoicePort>,
    pub tasks: Arc<app_tasks::TasksApp>,
    pub personas: Arc<dyn personas_contract::Personas>,
    pub stack: Option<&'a super::AgentStack>,
    pub updater: Arc<updater_impl::FsUpdater>,
    pub files: app_files::FilesDeps,
}

impl Built {
    /// Składa część i wypełnia gniazda zdrowia jej modułów.
    pub(crate) async fn work_stack(&self, d: WorkDepsIn<'_>) -> WorkStack {
        let engine = self.extra.broker.clone().map(|k| k.broker);
        let gui = Arc::new(GuiApp::new(d.monitor, engine.clone(), d.broker));
        let probe = d.options.cli_probe.clone().unwrap_or_else(|| {
            Arc::new(accounts_hub_impl::SystemCliProbe::from_env())
                as Arc<dyn accounts_hub_contract::CliProbe>
        });
        let home = d
            .paths
            .user_root
            .parent()
            .map_or_else(|| d.paths.user_root.clone(), std::path::Path::to_path_buf);
        let terminal = TerminalApp::new(d.options.pty.clone(), Programs::detect(Some(probe)), home)
            .with_bus(d.bus.clone(), tokio::runtime::Handle::current());
        let catalog = d.stack.map_or_else(Vec::new, |s| {
            s.tools.all().iter().map(|t| t.manifest().clone()).collect()
        });
        let work = app_skills::open(WorkDeps {
            catalog,
            local: d.paths.local.clone(),
            bus: d.bus.clone(),
            tasks: d.tasks,
            shell: d.shell.clone(),
            personas: d.personas,
            broker: engine,
            voice: d.voice,
        })
        .await;
        if let (Some(stack), Ok(module)) = (d.stack, work.skills.module()) {
            stack.launch.bind_skills(module);
        }
        let health = HealthApp::open(HealthDeps {
            local: d.paths.local.clone(),
            config: d.kernel.config.clone(),
            registry: d.registry,
            bus: d.bus.clone(),
            events: d.kernel.events.clone(),
            kernel_roots: vec![
                app_modules::broker::dev_dir(&d.paths.local),
                d.paths.local.join("versions"),
            ],
            evals_root: None,
            runner: None,
            scan_every_ms: None,
        })
        .await;
        let updates = app_updates::UpdatesApp::open(app_updates::UpdatesDeps {
            updater: d.updater,
            feed: None,
            config: d.kernel.config.clone(),
            events: Some(d.kernel.events.clone()),
            shell: d.shell.clone(),
            version: d.options.app_version.clone(),
            launcher: None,
            options: updater_impl::ServiceOptions::default(),
        });
        let plugins = d.stack.map(|s| s.tools.plugins()).unwrap_or_else(|| {
            Arc::new(app_plugins::PluginsApp::unavailable(
                "brak Brokera albo dziennika cofania",
            ))
        });
        let embed = self.search.clone().map(|search| app_models::EmbedDeps {
            search,
            lexical: Arc::new(app_modules::embedder::LexicalEmbedder),
            residency: (self.extra.residency.as_ref())
                .map(|r| r.manager() as Arc<dyn model_residency_contract::Residency>),
            scopes: self
                .memory
                .as_ref()
                .map(|m| m.service().backend().dbs().clone()),
            config: d.kernel.config.clone(),
        });
        let (low, high) = app_models::PARALLEL_RANGE;
        let parallel = super::memory::setting(d.kernel, app_models::PARALLEL_KEY)
            .await
            .and_then(|v| v.as_u64())
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| (low..=high).contains(n));
        let defaults = app_models::ModelsOptions::default();
        let models = app_models::ModelsApp::open(app_models::ModelsDeps {
            paths: d.paths.clone(),
            catalog: app_models::builtin(),
            events: Some(d.kernel.events.clone()),
            embed,
            options: app_models::ModelsOptions {
                parallel: parallel.unwrap_or(defaults.parallel),
                ..defaults
            },
        });
        if let (Some(docs), Some(a)) = (&self.extra.artifact_docs, &self.artifacts) {
            docs.bind(a.clone(), d.files.sessions.clone());
        }
        let stack = WorkStack {
            files: app_files::FilesApp::open(d.files),
            models,
            plugins,
            updates,
            gui,
            terminal: Arc::new(terminal),
            skills: work.skills.clone(),
            builder: work.builder.clone(),
            health,
        };
        for (id, slot) in &self.work_slots {
            fill(id, slot, &stack, &work);
        }
        stack
    }
}

fn fill(id: &str, slot: &HealthSlot, stack: &WorkStack, work: &app_skills::Work) {
    let f: Arc<dyn Fn() -> HealthStatus + Send + Sync> = match id {
        "skills" | "agent-builder" => {
            let (w, id) = (work.clone(), id.to_owned());
            Arc::new(move || w.health(&id))
        }
        "diagnostician" | "improver" | "evals" => {
            let (h, id) = (Arc::downgrade(&stack.health), id.to_owned());
            Arc::new(move || {
                h.upgrade()
                    .map_or(HealthStatus::NotStarted, |h| h.health(&id))
            })
        }
        "plugin-runtime" => {
            let p = stack.plugins.clone();
            Arc::new(move || p.health())
        }
        _ => Arc::new(|| HealthStatus::Healthy),
    };
    let _ = slot.set(f);
}
