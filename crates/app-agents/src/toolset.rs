//! Rejestr narzędzi agentek w aplikacji: `tools-fs` (11), `tools-shell` (2), `tools-clipboard`
//! (2, gdy jest schowek) nad jednym Brokerem i jednym dziennikiem cofania. Narzędzia same proszą
//! Brokera o tokeny przy każdym wywołaniu; ten moduł tylko je składa i filtruje rolami.

use std::sync::Arc;

use compliance_contract::{DenyLists, PathEnv};
use core_bus_contract::EventBus;
use personas_contract::Role;
use platform_contract::{ClipboardPort, ExecPort, FsPort};
use safety_broker_contract::Broker;
use tools_clipboard_contract::{ClipboardToolsConfig, ClipboardUndo, ClipboardUndoError};
use tools_clipboard_impl::{ClipboardTools, ClipboardToolsDeps};
use tools_common_contract::{Tool, ToolManifest, Toolset};
use tools_fs_contract::FsToolsConfig;
use tools_fs_impl::{FsTools, FsToolsDeps};
use tools_shell_contract::ShellToolsConfig;
use tools_shell_impl::{ShellTools, ShellToolsDeps};
use undo_journal_contract::UndoJournal;
use watchdog_contract::JobRegistry;

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
}

/// Narzędzia agentek (współdzielone przez wszystkie przebiegi).
#[derive(Clone)]
pub struct AgentTools {
    tools: Vec<Arc<dyn Tool>>,
    clipboard: Option<ClipboardTools>,
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
        let mut tools = fs.tools();
        tools.extend(shell.tools());
        if let Some(c) = &clipboard {
            tools.extend(c.tools());
        }
        tools.extend(deps.extra);
        Self { tools, clipboard }
    }

    /// Wszystkie narzędzia.
    pub fn all(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.clone()
    }

    /// Nazwy wszystkich narzędzi (dla `RunSpec::tools`).
    pub fn names(&self) -> Vec<String> {
        self.tools
            .iter()
            .map(|t| t.manifest().name.clone())
            .collect()
    }

    /// Manifest narzędzia po nazwie.
    pub fn manifest(&self, name: &str) -> Option<&ToolManifest> {
        self.tools
            .iter()
            .map(|t| t.manifest())
            .find(|m| m.name == name)
    }

    /// Narzędzia, które przysługują agentce o tych rolach (grupy narzędzi ról; rola tylko do
    /// odczytu nie dostaje narzędzi zmieniających stan) — ta sama reguła co rejestr runtime.
    pub fn allowed_for(&self, roles: &[Role]) -> Vec<String> {
        let groups: Vec<String> = roles.iter().flat_map(|r| r.tools.clone()).collect();
        let read_only = !roles.is_empty() && roles.iter().all(|r| r.read_only);
        self.tools
            .iter()
            .map(|t| t.manifest())
            .filter(|m| m.allowed_for(&groups, read_only))
            .map(|m| m.name.clone())
            .collect()
    }

    /// Cofa zapis schowka (karta „Cofnij"); bez narzędzi schowka — nieznany krok.
    pub fn undo_clipboard(&self, id: u64) -> Result<(), ClipboardUndoError> {
        match &self.clipboard {
            Some(c) => c.undo(id),
            None => Err(ClipboardUndoError::Unknown(id)),
        }
    }
}
