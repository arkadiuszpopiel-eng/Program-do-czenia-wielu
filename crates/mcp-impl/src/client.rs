//! Klient MCP serwera zewnętrznego uruchamianego przez stdio: `initialize` z negocjacją wersji,
//! `tools/list` z odciskami i oceną zaufania, `tools/call` tylko z aktualną zgodą.
//! Lista narzędzi jest odświeżana **przed każdym wywołaniem** — zmiana opisu bez powiadomienia
//! (rug pull) też zostaje wykryta i blokuje wywołanie.

use std::collections::BTreeMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;
use mcp_contract::protocol::{
    InitializeResult, LATEST_PROTOCOL_VERSION, ListToolsResult, is_supported_version, methods,
};
use mcp_contract::{
    CallToolResult, ConsentOrigin, Implementation, McpClient, McpError, McpServerConfig,
    McpWarning, PinStore, ToolAssessment, ToolFingerprint, ToolPin, ToolState, assess,
};
use serde_json::{Value, json};

use crate::connection::Connection;

/// Zmienne dziedziczone przez serwer MCP (reszta środowiska Alfy, w tym klucze, nie przechodzi).
pub const SERVER_ENV_ALLOWLIST: [&str; 14] = [
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "TEMP",
    "TMP",
    "TMPDIR",
    "HOME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "LANG",
];

/// Maksymalna liczba stron `tools/list`.
const MAX_LIST_PAGES: usize = 20;

/// Klient MCP (stdio).
pub struct StdioMcpClient {
    config: McpServerConfig,
    conn: Connection,
    child: tokio::sync::Mutex<Option<tokio::process::Child>>,
    init: InitializeResult,
    pins: Arc<dyn PinStore>,
    tools: Mutex<Vec<ToolAssessment>>,
}

impl StdioMcpClient {
    /// Uruchamia serwer z konfiguracji i inicjalizuje sesję.
    pub async fn spawn(config: McpServerConfig, pins: Arc<dyn PinStore>) -> Result<Self, McpError> {
        config.validate()?;
        let mut cmd = tokio::process::Command::new(&config.command);
        cmd.args(&config.args)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        for name in SERVER_ENV_ALLOWLIST {
            if let Some(v) = std::env::var_os(name) {
                cmd.env(name, v);
            }
        }
        cmd.envs(&config.env);
        if let Some(cwd) = &config.cwd {
            cmd.current_dir(cwd);
        }
        let mut child = cmd.spawn().map_err(|e| McpError::Spawn(e.to_string()))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(McpError::Spawn("brak potoków stdio".into()));
        };
        let conn = Connection::new(stdout, stdin);
        Self::initialize(config, conn, Some(child), pins).await
    }

    /// Klient nad gotowym strumieniem (serwer w procesie — testy, serwery wbudowane).
    pub async fn connect<R, W>(
        config: McpServerConfig,
        reader: R,
        writer: W,
        pins: Arc<dyn PinStore>,
    ) -> Result<Self, McpError>
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static,
        W: tokio::io::AsyncWrite + Send + Unpin + 'static,
    {
        config.validate()?;
        Self::initialize(config, Connection::new(reader, writer), None, pins).await
    }

    async fn initialize(
        config: McpServerConfig,
        conn: Connection,
        child: Option<tokio::process::Child>,
        pins: Arc<dyn PinStore>,
    ) -> Result<Self, McpError> {
        let timeout = Duration::from_millis(config.request_timeout_ms);
        let params = json!({
            "protocolVersion": LATEST_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "alfa", "title": "Alfa", "version": env!("CARGO_PKG_VERSION")},
        });
        let raw = conn.request(methods::INITIALIZE, params, timeout).await?;
        let init: InitializeResult = serde_json::from_value(raw)
            .map_err(|e| McpError::Protocol(format!("niepoprawny wynik `initialize`: {e}")))?;
        if !is_supported_version(&init.protocol_version) {
            conn.close().await;
            return Err(McpError::UnsupportedProtocolVersion(init.protocol_version));
        }
        conn.notify(methods::INITIALIZED, None).await?;
        Ok(Self {
            config,
            conn,
            child: tokio::sync::Mutex::new(child),
            init,
            pins,
            tools: Mutex::new(Vec::new()),
        })
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(self.config.request_timeout_ms)
    }

    fn cache(&self) -> MutexGuard<'_, Vec<ToolAssessment>> {
        self.tools.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Instrukcje serwera z `initialize` — treść niezaufana.
    pub fn instructions(&self) -> Option<&str> {
        self.init.instructions.as_deref()
    }

    async fn refresh(&self) -> Result<Vec<ToolAssessment>, McpError> {
        let _ = self.conn.take_tools_changed();
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_LIST_PAGES {
            let params = cursor
                .as_ref()
                .map_or_else(|| json!({}), |c| json!({"cursor": c}));
            let raw = self
                .conn
                .request(methods::TOOLS_LIST, params, self.timeout())
                .await?;
            let page: ListToolsResult = serde_json::from_value(raw)
                .map_err(|e| McpError::Protocol(format!("niepoprawny wynik `tools/list`: {e}")))?;
            tools.extend(page.tools);
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        let assessed: Vec<ToolAssessment> = tools
            .iter()
            .map(|t| assess(&self.config.id, self.config.trust, t, self.pins.as_ref()))
            .collect();
        let previous: BTreeMap<String, ToolFingerprint> = self
            .cache()
            .iter()
            .map(|a| (a.tool.name.clone(), a.fingerprint.clone()))
            .collect();
        for a in &assessed {
            if let ToolState::NeedsConsent(mcp_contract::ConsentReason::Changed { approved }) =
                &a.state
                && previous.get(&a.tool.name) != Some(&a.fingerprint)
            {
                tracing::warn!(serwer = %self.config.id, narzedzie = %a.tool.name,
                    "definicja narzędzia MCP zmieniła się po zatwierdzeniu — zablokowane");
                self.conn.warn(McpWarning::ToolDefinitionChanged {
                    tool: a.tool.name.clone(),
                    approved: approved.clone(),
                    current: a.fingerprint.clone(),
                });
            }
            if a.untrusted && previous.get(&a.tool.name) != Some(&a.fingerprint) {
                self.conn.warn(McpWarning::SuspiciousTool {
                    tool: a.tool.name.clone(),
                    signals: a.signals.clone(),
                });
            }
        }
        *self.cache() = assessed.clone();
        Ok(assessed)
    }
}

#[async_trait]
impl McpClient for StdioMcpClient {
    fn server_id(&self) -> &str {
        &self.config.id
    }

    fn server_info(&self) -> Option<Implementation> {
        Some(self.init.server_info.clone())
    }

    fn protocol_version(&self) -> Option<String> {
        Some(self.init.protocol_version.clone())
    }

    async fn tools(&self) -> Result<Vec<ToolAssessment>, McpError> {
        let cached = self.cache().clone();
        if cached.is_empty() || self.conn.take_tools_changed() {
            return self.refresh().await;
        }
        Ok(cached)
    }

    async fn consent_tool(
        &self,
        tool: &str,
        fingerprint: &ToolFingerprint,
        origin: ConsentOrigin,
    ) -> Result<(), McpError> {
        if origin != ConsentOrigin::User {
            return Err(McpError::ConsentNotPermitted);
        }
        let current = self.refresh().await?;
        let found = current
            .iter()
            .find(|a| a.tool.name == tool)
            .ok_or_else(|| McpError::UnknownTool(tool.to_owned()))?;
        if &found.fingerprint != fingerprint {
            return Err(McpError::FingerprintMismatch(tool.to_owned()));
        }
        self.pins.put(ToolPin {
            server: self.config.id.clone(),
            tool: tool.to_owned(),
            fingerprint: fingerprint.clone(),
        });
        for a in self.cache().iter_mut().filter(|a| a.tool.name == tool) {
            a.state = ToolState::Approved;
        }
        Ok(())
    }

    async fn call_tool(&self, tool: &str, arguments: Value) -> Result<CallToolResult, McpError> {
        let current = self.refresh().await?;
        let found = current
            .iter()
            .find(|a| a.tool.name == tool)
            .ok_or_else(|| McpError::UnknownTool(tool.to_owned()))?;
        if let ToolState::NeedsConsent(reason) = &found.state {
            return Err(McpError::NeedsConsent {
                tool: tool.to_owned(),
                reason: reason.clone(),
            });
        }
        let raw = self
            .conn
            .request(
                methods::TOOLS_CALL,
                json!({"name": tool, "arguments": arguments}),
                self.timeout(),
            )
            .await?;
        serde_json::from_value(raw)
            .map_err(|e| McpError::Protocol(format!("niepoprawny wynik `tools/call`: {e}")))
    }

    fn take_warnings(&self) -> Vec<McpWarning> {
        self.conn.take_warnings()
    }

    async fn shutdown(&self) {
        self.conn.close().await;
        if let Some(mut child) = self.child.lock().await.take() {
            // Po zamknięciu stdin serwer powinien zakończyć się sam; inaczej — zabijamy.
            if tokio::time::timeout(Duration::from_secs(2), child.wait())
                .await
                .is_err()
            {
                let _ = child.kill().await;
            }
        }
    }
}
