//! Rejestr narzędzi agentek w aplikacji: `tools-fs` (11), `tools-shell` (2), `tools-clipboard`
//! (2, gdy jest schowek), F6: `tools-office` (2), `tools-browser` (6), `tools-system` (9)
//! i `tools-net` (2, `net_search` po podpięciu dostawcy), F8: narzędzia aktywnych
//! wtyczek Wasm (`plugin_*`, liczone przy każdym odczycie — po instalacji/wyłączeniu od razu
//! aktualne) nad jednym Brokerem i jednym dziennikiem cofania. Narzędzia same proszą Brokera
//! o tokeny przy każdym wywołaniu; ten moduł tylko je składa i filtruje rolami.

use std::sync::Arc;

use app_plugins::{PluginsApp, PluginsDeps};
use compliance_contract::{DenyLists, PathEnv};
use core_bus_contract::EventBus;
use personas_contract::Role;
use platform_contract::{ClipboardPort, ExecPort, FsPort};
use safety_broker_contract::Broker;
use tools_browser_contract::BrowserToolsConfig;
use tools_browser_impl::{BrowserTools, BrowserToolsDeps};
use tools_clipboard_contract::{ClipboardToolsConfig, ClipboardUndo, ClipboardUndoError};
use tools_clipboard_impl::{ClipboardTools, ClipboardToolsDeps};
use tools_common_contract::{Tool, ToolManifest, Toolset};
use tools_fs_contract::FsToolsConfig;
use tools_fs_impl::{FsTools, FsToolsDeps};
use tools_office_contract::OfficeToolsConfig;
use tools_office_impl::{OfficeTools, OfficeToolsDeps};
use tools_shell_contract::ShellToolsConfig;
use tools_shell_impl::{ShellTools, ShellToolsDeps};
use undo_journal_contract::UndoJournal;
use watchdog_contract::JobRegistry;

use crate::apps::AppsDeps;

/// Zależności narzędzi.
#[derive(Clone)]
pub struct ToolsDeps {
    /// Broker (tokeny, zatwierdzenia, taint) — zwykle [`crate::TicketLog`] nad Brokerem.
    pub broker: Arc<dyn Broker>,
    /// Dziennik cofania (mutacje plików, snapshot zakresu powłoki).
    pub journal: Arc<dyn UndoJournal>,
    /// System plików.
    pub fs: Arc<dyn FsPort>,
    /// Uruchamianie poleceń w Job Object.
    pub exec: Arc<dyn ExecPort>,
    /// Schowek (`None` — bez narzędzi schowka).
    pub clipboard: Option<Arc<dyn ClipboardPort>>,
    /// Środowisko ścieżek (profil właściciela).
    pub env: PathEnv,
    /// Deny-listy Jądra.
    pub deny: DenyLists,
    /// Rejestr Job Objects kill-switcha (Broker).
    pub jobs: Option<Arc<dyn JobRegistry>>,
    /// Magistrala (`tool.*`).
    pub bus: Option<Arc<dyn EventBus>>,
    /// Konfiguracja powłoki (ścieżki `pwsh`/`cmd`, limity).
    pub shell: ShellToolsConfig,
    /// Środowisko bazowe procesów (`None` = środowisko Alfy, filtrowane allowlistą).
    pub base_env: Option<Vec<(String, String)>>,
    /// Narzędzia spoza `tools-*` (np. pamięć: `memory_recall`, `memory_remember`).
    pub extra: Vec<Arc<dyn Tool>>,
    /// Word/Excel, przeglądarka i wtyczki (`None` — bez tych narzędzi).
    pub apps: Option<AppsDeps>,
}

/// Narzędzia agentek (współdzielone przez wszystkie przebiegi).
#[derive(Clone)]
pub struct AgentTools {
    tools: Vec<Arc<dyn Tool>>,
    clipboard: Option<ClipboardTools>,
    browser: Option<BrowserTools>,
    plugins: Option<Arc<PluginsApp>>,
    system: Option<tools_system_impl::SystemTools>,
}

impl std::fmt::Debug for AgentTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentTools")
            .field("tools", &self.names())
            .finish_non_exhaustive()
    }
}

impl AgentTools {
    /// Składa zestawy narzędzi.
    pub fn new(deps: ToolsDeps) -> Self {
        let (mut tools, browser, plugins) = match &deps.apps {
            Some(apps) => apps_tools(&deps, apps),
            None => (Vec::new(), None, None),
        };
        let system = match deps.apps.as_ref().and_then(|a| a.sysnet.as_ref()) {
            Some(s) => {
                let (more, system) = crate::sysnet::sysnet_tools(&deps, s);
                tools.extend(more);
                Some(system)
            }
            None => None,
        };
        let fs = FsTools::new(FsToolsDeps {
            fs: deps.fs.clone(),
            journal: deps.journal.clone(),
            broker: deps.broker.clone(),
            env: deps.env.clone(),
            deny: deps.deny.clone(),
            config: FsToolsConfig::default(),
            bus: deps.bus.clone(),
        });
        let shell = ShellTools::new(ShellToolsDeps {
            exec: deps.exec,
            journal: deps.journal,
            broker: deps.broker.clone(),
            env: deps.env.clone(),
            deny: deps.deny.clone(),
            config: deps.shell,
            base_env: deps.base_env,
            jobs: deps.jobs,
            bus: deps.bus.clone(),
        });
        let clipboard = deps.clipboard.map(|port| {
            ClipboardTools::new(ClipboardToolsDeps {
                clipboard: port,
                broker: deps.broker,
                env: deps.env,
                deny: deps.deny,
                config: ClipboardToolsConfig::default(),
                bus: deps.bus,
            })
        });
        let mut base = fs.tools();
        base.extend(shell.tools());
        if let Some(c) = &clipboard {
            base.extend(c.tools());
        }
        base.append(&mut tools);
        base.extend(deps.extra);
        Self {
            tools: base,
            clipboard,
            browser,
            plugins,
            system,
        }
    }

    /// Wszystkie narzędzia (z bieżącymi narzędziami aktywnych wtyczek).
    pub fn all(&self) -> Vec<Arc<dyn Tool>> {
        let mut all = self.tools.clone();
        if let Some(p) = &self.plugins {
            all.extend(p.tools());
        }
        all
    }

    /// Nazwy wszystkich narzędzi (dla `RunSpec::tools`).
    pub fn names(&self) -> Vec<String> {
        self.all()
            .iter()
            .map(|t| t.manifest().name.clone())
            .collect()
    }

    /// Manifest narzędzia po nazwie.
    pub fn manifest(&self, name: &str) -> Option<ToolManifest> {
        self.all()
            .iter()
            .map(|t| t.manifest())
            .find(|m| m.name == name)
            .cloned()
    }

    /// Wtyczki (komendy strony „Wtyczki”); bez portów aplikacji — niedostępne z powodem.
    pub fn plugins(&self) -> Arc<PluginsApp> {
        self.plugins.clone().unwrap_or_else(|| {
            Arc::new(PluginsApp::unavailable(
                "brak katalogu wtyczek albo portów aplikacji",
            ))
        })
    }

    /// Kill-switch: zamyka wszystkie przeglądarki agentek (zgody hostów wygasają). Zwraca liczbę
    /// zamkniętych sesji przeglądarki.
    pub fn kill_switch(&self) -> usize {
        self.browser.as_ref().map_or(0, BrowserTools::close_all)
    }

    /// Narzędzia, które przysługują agentce o tych rolach (grupy narzędzi ról; rola tylko do
    /// odczytu nie dostaje narzędzi zmieniających stan) — ta sama reguła co rejestr runtime.
    pub fn allowed_for(&self, roles: &[Role]) -> Vec<String> {
        let groups: Vec<String> = roles.iter().flat_map(|r| r.tools.clone()).collect();
        let read_only = !roles.is_empty() && roles.iter().all(|r| r.read_only);
        self.all()
            .iter()
            .map(|t| t.manifest())
            .filter(|m| m.allowed_for(&groups, read_only))
            .map(|m| m.name.clone())
            .collect()
    }

    /// Cofa zapis zmiennej użytkownika przez `system_env_set` (krok `undo_id`); bez narzędzi
    /// systemowych — nieznany krok. Zwraca opis dla UI.
    pub fn undo_env(&self, id: u64) -> Result<String, tools_system_impl::EnvUndoError> {
        match &self.system {
            Some(s) => s.undo_env(id),
            None => Err(tools_system_impl::EnvUndoError::Unknown(id)),
        }
    }

    /// Cofa zapis schowka (karta „Cofnij"); bez narzędzi schowka — nieznany krok.
    pub fn undo_clipboard(&self, id: u64) -> Result<(), ClipboardUndoError> {
        match &self.clipboard {
            Some(c) => c.undo(id),
            None => Err(ClipboardUndoError::Unknown(id)),
        }
    }
}

/// Narzędzia aplikacji: Office + przeglądarka (lista), zestaw przeglądarki (kill-switch), wtyczki.
type AppsTools = (
    Vec<Arc<dyn Tool>>,
    Option<BrowserTools>,
    Option<Arc<PluginsApp>>,
);

/// Office (`office_read`, `office_edit`), przeglądarka (`browser_*`) i wtyczki nad portami aplikacji.
fn apps_tools(deps: &ToolsDeps, apps: &AppsDeps) -> AppsTools {
    let office = OfficeTools::new(OfficeToolsDeps {
        fs: deps.fs.clone(),
        office: apps.office.clone(),
        journal: deps.journal.clone(),
        broker: deps.broker.clone(),
        env: deps.env.clone(),
        deny: deps.deny.clone(),
        config: OfficeToolsConfig::default(),
        bus: deps.bus.clone(),
    });
    let browser = BrowserTools::new(BrowserToolsDeps {
        browser: apps.browser.clone(),
        broker: deps.broker.clone(),
        spec: apps.browser_spec.clone(),
        deny: deps.deny.clone(),
        env: deps.env.clone(),
        config: BrowserToolsConfig::default(),
        bus: deps.bus.clone(),
    });
    let plugins = apps.plugins_dir.as_ref().map(|dir| {
        Arc::new(PluginsApp::new(PluginsDeps {
            broker: deps.broker.clone(),
            fs: deps.fs.clone(),
            journal: deps.journal.clone(),
            env: deps.env.clone(),
            deny: deps.deny.clone(),
            bus: deps.bus.clone(),
            dir: dir.clone(),
            net: None,
        }))
    });
    let mut tools = office.tools();
    tools.extend(browser.tools());
    (tools, Some(browser), plugins)
}
