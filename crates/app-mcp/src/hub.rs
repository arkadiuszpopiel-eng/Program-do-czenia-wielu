//! Menedżer serwerów MCP: połączenia uruchamiane przy pierwszym użyciu, stan ostatniej próby,
//! lista narzędzi z oceną zgody i wywołania. Zerwane połączenie (proces serwera zakończony) jest
//! zapominane — następne użycie uruchamia serwer od nowa.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use accounts_hub_contract::{CliProbe, SecretName, SecretStore};
use async_trait::async_trait;
use mcp_contract::{
    CallToolResult, ConsentOrigin, McpClient, McpError, McpServerConfig, PinStore, ToolAssessment,
    ToolFingerprint, ToolState, assess,
};
use mcp_impl::StdioMcpClient;
use serde_json::Value;

use crate::store::{FilePinStore, PINS_FILE, SERVERS_FILE, ServerEntry, load_servers};

/// Uruchamia klienta MCP dla konfiguracji (w aplikacji — proces przez stdio; w testach — serwer
/// w procesie).
#[async_trait]
pub trait Connector: Send + Sync {
    /// Połączenie z zainicjowaną sesją.
    async fn connect(
        &self,
        config: McpServerConfig,
        pins: Arc<dyn PinStore>,
    ) -> Result<Arc<dyn McpClient>, McpError>;
}

/// Serwer jako proces potomny (stdio, czyste środowisko z listą dozwoloną).
pub struct StdioConnector;

#[async_trait]
impl Connector for StdioConnector {
    async fn connect(
        &self,
        config: McpServerConfig,
        pins: Arc<dyn PinStore>,
    ) -> Result<Arc<dyn McpClient>, McpError> {
        Ok(Arc::new(StdioMcpClient::spawn(config, pins).await?))
    }
}

/// Zależności menedżera.
pub struct McpHubDeps {
    /// Katalog konfiguracji (`mcp-servers.json`, `mcp-pins.json`).
    pub config_dir: PathBuf,
    /// Katalog roboczy serwerów bez własnego (`<dane lokalne>\mcp\<id>`).
    pub work_dir: PathBuf,
    /// Magazyn sekretów (zmienne tajne); `None` — zmiennych tajnych nie da się zapisać.
    pub secrets: Option<Arc<dyn SecretStore>>,
    /// Wyszukiwanie programów w PATH (z PATHEXT: `npx` → `npx.cmd`).
    pub probe: Option<Arc<dyn CliProbe>>,
    /// Sposób łączenia.
    pub connector: Arc<dyn Connector>,
}

/// Stan ostatniej próby połączenia z serwerem.
#[derive(Debug, Clone, Default)]
pub(crate) struct Status {
    pub error: Option<String>,
    pub info: Option<String>,
    pub tools: Vec<ToolAssessment>,
}

/// Menedżer serwerów MCP użytkownika.
pub struct McpHub {
    pub(crate) servers_path: PathBuf,
    pub(crate) work_dir: PathBuf,
    pub(crate) pins: Arc<FilePinStore>,
    pub(crate) secrets: Option<Arc<dyn SecretStore>>,
    pub(crate) probe: Option<Arc<dyn CliProbe>>,
    connector: Arc<dyn Connector>,
    pub(crate) entries: Mutex<Vec<ServerEntry>>,
    live: tokio::sync::Mutex<BTreeMap<String, Arc<dyn McpClient>>>,
    status: Mutex<BTreeMap<String, Status>>,
}

/// Nazwa sekretu zmiennej serwera.
pub(crate) fn secret_name(server: &str, var: &str) -> Option<SecretName> {
    SecretName::new(format!("mcp/{server}/{var}")).ok()
}

impl McpHub {
    /// Menedżer z listą serwerów i zgodami z katalogu konfiguracji.
    pub fn new(deps: McpHubDeps) -> Self {
        let servers_path = deps.config_dir.join(SERVERS_FILE);
        let entries = load_servers(&servers_path);
        Self {
            pins: Arc::new(FilePinStore::open(deps.config_dir.join(PINS_FILE))),
            servers_path,
            work_dir: deps.work_dir,
            secrets: deps.secrets,
            probe: deps.probe,
            connector: deps.connector,
            entries: Mutex::new(entries),
            live: tokio::sync::Mutex::new(BTreeMap::new()),
            status: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) fn lock_entries(&self) -> MutexGuard<'_, Vec<ServerEntry>> {
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub(crate) fn lock_status(&self) -> MutexGuard<'_, BTreeMap<String, Status>> {
        self.status.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Wpis serwera.
    pub(crate) fn entry(&self, id: &str) -> Option<ServerEntry> {
        self.lock_entries().iter().find(|e| e.id == id).cloned()
    }

    /// Wpisy włączonych serwerów.
    pub(crate) fn enabled(&self) -> Vec<ServerEntry> {
        self.lock_entries()
            .iter()
            .filter(|e| e.enabled)
            .cloned()
            .collect()
    }

    /// Ścieżka programu: bez katalogu — szukany w PATH; z katalogiem — musi istnieć.
    pub(crate) fn resolve(&self, command: &str) -> Option<PathBuf> {
        let path = Path::new(command);
        if path.components().count() > 1 || path.is_absolute() {
            return path.is_file().then(|| path.to_path_buf());
        }
        self.probe.as_ref()?.locate(command)
    }

    fn config_for(&self, e: &ServerEntry) -> Result<McpServerConfig, String> {
        let command = self.resolve(&e.command).ok_or_else(|| {
            format!(
                "Nie znaleziono programu `{}`. Zainstaluj go (np. Node.js dla `npx`, uv dla `uvx`) \
                 albo podaj pełną ścieżkę.",
                e.command
            )
        })?;
        let mut env = e.env.clone();
        for var in &e.secret_env {
            let value =
                secret_name(&e.id, var).and_then(|n| self.secrets.as_ref()?.get(&n).ok().flatten());
            let Some(value) = value else {
                return Err(format!(
                    "Brak wartości tajnej zmiennej {var} — wpisz ją ponownie w ustawieniach serwera."
                ));
            };
            env.insert(var.clone(), value.expose_secret().to_owned());
        }
        let cwd = match &e.cwd {
            Some(c) => PathBuf::from(c),
            None => {
                let dir = self.work_dir.join(&e.id);
                std::fs::create_dir_all(&dir)
                    .map_err(|err| format!("Nie utworzono katalogu roboczego serwera: {err}"))?;
                dir
            }
        };
        Ok(McpServerConfig {
            id: e.id.clone(),
            command,
            args: e.args.clone(),
            env,
            cwd: Some(cwd),
            trust: e.trust,
            request_timeout_ms: e.timeout_ms,
        })
    }

    /// Połączenie z serwerem (istniejące albo nowe). Wyłączony serwer — błąd.
    pub(crate) async fn client(&self, id: &str) -> Result<Arc<dyn McpClient>, String> {
        let entry = self
            .entry(id)
            .ok_or_else(|| format!("Nie ma serwera MCP `{id}`."))?;
        if !entry.enabled {
            return Err(format!(
                "Serwer MCP „{}” jest wyłączony w ustawieniach.",
                entry.name
            ));
        }
        let mut live = self.live.lock().await;
        if let Some(c) = live.get(id) {
            return Ok(c.clone());
        }
        let result = match self.config_for(&entry) {
            Ok(config) => self
                .connector
                .connect(config, self.pins.clone())
                .await
                .map_err(|e| describe(&e)),
            Err(e) => Err(e),
        };
        let mut status = self.lock_status();
        let slot = status.entry(id.to_owned()).or_default();
        match result {
            Ok(client) => {
                slot.error = None;
                slot.info = client
                    .server_info()
                    .map(|i| format!("{} {}", i.name, i.version));
                live.insert(id.to_owned(), client.clone());
                Ok(client)
            }
            Err(e) => {
                slot.error = Some(e.clone());
                Err(e)
            }
        }
    }

    /// Zamyka połączenie (zmiana ustawień, usunięcie, zerwanie).
    pub(crate) async fn disconnect(&self, id: &str) {
        let client = self.live.lock().await.remove(id);
        if let Some(c) = client {
            c.shutdown().await;
        }
    }

    /// Czy serwer jest połączony.
    pub(crate) async fn connected(&self, id: &str) -> bool {
        self.live.lock().await.contains_key(id)
    }

    /// Błąd operacji: zerwane połączenie jest zapominane, stan zapisany.
    async fn failed(&self, id: &str, e: &McpError) -> String {
        if matches!(
            e,
            McpError::Closed | McpError::Transport(_) | McpError::Timeout { .. }
        ) {
            self.disconnect(id).await;
        }
        let text = describe(e);
        self.lock_status().entry(id.to_owned()).or_default().error = Some(text.clone());
        text
    }

    /// Narzędzia serwera z oceną (łączy, gdy trzeba).
    pub async fn tools_of(&self, id: &str) -> Result<Vec<ToolAssessment>, String> {
        let client = self.client(id).await?;
        match client.tools().await {
            Ok(tools) => {
                // Lista bywa z pamięci klienta — stan zgód oceniany na nowo (cofnięta zgoda).
                let trust = self.entry(id).map(|e| e.trust).unwrap_or_default();
                let tools: Vec<ToolAssessment> = tools
                    .iter()
                    .map(|a| {
                        let mut now = assess(id, trust, &a.tool, self.pins.as_ref());
                        if a.state == ToolState::AutoApproved && now.state == ToolState::Approved {
                            now.state = ToolState::AutoApproved;
                        }
                        now
                    })
                    .collect();
                let mut status = self.lock_status();
                let slot = status.entry(id.to_owned()).or_default();
                slot.error = None;
                slot.tools = tools.clone();
                Ok(tools)
            }
            Err(e) => Err(self.failed(id, &e).await),
        }
    }

    /// Zgoda użytkownika na narzędzie w konkretnej wersji opisu.
    pub async fn approve_tool(
        &self,
        id: &str,
        tool: &str,
        fingerprint: &str,
    ) -> Result<(), String> {
        let fp = ToolFingerprint::parse(fingerprint)
            .ok_or_else(|| "Niepoprawny odcisk narzędzia — odśwież listę.".to_owned())?;
        let client = self.client(id).await?;
        client
            .consent_tool(tool, &fp, ConsentOrigin::User)
            .await
            .map_err(|e| describe(&e))
    }

    /// Wywołanie narzędzia (tylko z aktualną zgodą — pilnuje klient).
    pub async fn call(&self, id: &str, tool: &str, args: Value) -> Result<CallToolResult, String> {
        let client = self.client(id).await?;
        match client.call_tool(tool, args).await {
            Ok(r) => Ok(r),
            Err(e) => Err(self.failed(id, &e).await),
        }
    }

    /// Stan ostatniej próby.
    pub(crate) fn status_of(&self, id: &str) -> Status {
        self.lock_status().get(id).cloned().unwrap_or_default()
    }

    /// Zamyka wszystkie połączenia (koniec pracy aplikacji, kill-switch).
    pub async fn shutdown_all(&self) {
        let all: Vec<_> = std::mem::take(&mut *self.live.lock().await)
            .into_values()
            .collect();
        for c in all {
            c.shutdown().await;
        }
    }
}

/// Czytelny opis błędu MCP dla użytkownika i agentki.
pub(crate) fn describe(e: &McpError) -> String {
    match e {
        McpError::Spawn(m) => format!("Nie udało się uruchomić serwera: {m}."),
        McpError::Closed => "Serwer zakończył działanie (sprawdź polecenie i argumenty).".into(),
        McpError::Timeout { method } => {
            format!("Serwer nie odpowiedział w limicie czasu (`{method}`).")
        }
        McpError::NeedsConsent { tool, .. } => {
            format!("Narzędzie `{tool}` czeka na zgodę użytkownika (Ustawienia → Serwery MCP).")
        }
        McpError::UnknownTool(t) => format!("Serwer nie ma narzędzia `{t}`."),
        other => other.to_string(),
    }
}
