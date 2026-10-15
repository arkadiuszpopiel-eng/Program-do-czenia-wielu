//! Przekład stanu menedżera na DTO interfejsu i polecenia IPC (lista, narzędzia, zgody).

use app_api::AppError;
use app_api::dto::{
    McpConnState, McpEnvVar, McpServerTools, McpServerView, McpToolState, McpToolView, McpTrust,
};
use mcp_contract::{ConsentReason, ToolAssessment, ToolState, TrustLevel};

use crate::hub::McpHub;
use crate::store::ServerEntry;

/// Najdłuższy opis narzędzia pokazywany w interfejsie.
const DESCRIPTION_MAX: usize = 600;

/// Poziom zaufania z DTO.
pub(crate) fn to_trust(t: McpTrust) -> TrustLevel {
    match t {
        McpTrust::Untrusted => TrustLevel::Untrusted,
        McpTrust::Reviewed => TrustLevel::Reviewed,
        McpTrust::Trusted => TrustLevel::Trusted,
    }
}

/// Poziom zaufania do DTO.
pub(crate) fn from_trust(t: TrustLevel) -> McpTrust {
    match t {
        TrustLevel::Untrusted => McpTrust::Untrusted,
        TrustLevel::Reviewed => McpTrust::Reviewed,
        TrustLevel::Trusted => McpTrust::Trusted,
    }
}

/// Tekst skrócony do `max` znaków (z wielokropkiem).
pub(crate) fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

/// Widok narzędzia.
pub(crate) fn tool_view(a: &ToolAssessment) -> McpToolView {
    let state = match &a.state {
        ToolState::Approved => McpToolState::Approved,
        ToolState::AutoApproved => McpToolState::AutoApproved,
        ToolState::NeedsConsent(ConsentReason::New) => McpToolState::New,
        ToolState::NeedsConsent(ConsentReason::Changed { .. }) => McpToolState::Changed,
    };
    let params = a
        .tool
        .input_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|p| p.keys().take(20).cloned().collect())
        .unwrap_or_default();
    McpToolView {
        name: a.tool.name.clone(),
        title: a.tool.title.as_deref().map(|t| clip(t, 120)),
        description: a
            .tool
            .description
            .as_deref()
            .map(|d| clip(d, DESCRIPTION_MAX)),
        state,
        fingerprint: a.fingerprint.as_str().to_owned(),
        suspicious: a.untrusted,
        params,
    }
}

impl McpHub {
    /// Widok serwera; `connected` — czy istnieje połączenie.
    pub(crate) fn view(&self, e: &ServerEntry, connected: bool) -> McpServerView {
        let status = self.status_of(&e.id);
        let state = if !e.enabled {
            McpConnState::Disabled
        } else if connected {
            McpConnState::Connected
        } else if status.error.is_some() {
            McpConnState::Failed
        } else {
            McpConnState::Idle
        };
        let env = e
            .env
            .iter()
            .map(|(name, value)| McpEnvVar {
                name: name.clone(),
                value: Some(value.clone()),
                secret: false,
            })
            .chain(e.secret_env.iter().map(|name| McpEnvVar {
                name: name.clone(),
                value: None,
                secret: true,
            }))
            .collect();
        let approved = status.tools.iter().filter(|t| t.state.callable()).count();
        McpServerView {
            id: e.id.clone(),
            name: e.name.clone(),
            command: e.command.clone(),
            resolved: self.resolve(&e.command).map(|p| p.display().to_string()),
            args: e.args.clone(),
            cwd: e.cwd.clone(),
            env,
            trust: from_trust(e.trust),
            enabled: e.enabled,
            state,
            error: if e.enabled { status.error } else { None },
            server_info: status.info,
            tools_total: u32::try_from(status.tools.len()).unwrap_or(u32::MAX),
            tools_approved: u32::try_from(approved).unwrap_or(u32::MAX),
            timeout_ms: e.timeout_ms,
        }
    }

    /// `mcp_list`.
    pub async fn list(&self) -> Vec<McpServerView> {
        let entries = self.lock_entries().clone();
        let mut out = Vec::with_capacity(entries.len());
        for e in &entries {
            out.push(self.view(e, self.connected(&e.id).await));
        }
        out
    }

    async fn tools_view(&self, id: &str) -> Result<McpServerTools, AppError> {
        let entry = self
            .entry(id)
            .ok_or_else(|| AppError::not_found("serwer MCP"))?;
        // Nieudana próba: stan „błąd” z powodem i ostatnia znana lista narzędzi.
        let tools = match self.tools_of(id).await {
            Ok(tools) => tools,
            Err(_) => self.status_of(id).tools,
        };
        Ok(McpServerTools {
            server: self.view(&entry, self.connected(id).await),
            tools: tools.iter().map(tool_view).collect(),
        })
    }

    /// `mcp_tools`: łączy (gdy serwer włączony) i zwraca narzędzia z oceną zgody.
    pub async fn tools_ipc(&self, id: &str) -> Result<McpServerTools, AppError> {
        self.tools_view(id).await
    }

    /// `mcp_approve`: zgoda użytkownika na narzędzie w wersji opisu o podanym odcisku.
    pub async fn approve_ipc(
        &self,
        id: &str,
        tool: &str,
        fingerprint: &str,
    ) -> Result<McpServerTools, AppError> {
        self.approve_tool(id, tool, fingerprint)
            .await
            .map_err(|e| AppError::invalid_field("tool", e))?;
        self.tools_view(id).await
    }

    /// `mcp_revoke`: cofa zgodę na narzędzie.
    pub async fn revoke_ipc(&self, id: &str, tool: &str) -> Result<McpServerTools, AppError> {
        if self.entry(id).is_none() {
            return Err(AppError::not_found("serwer MCP"));
        }
        self.pins.revoke(id, tool);
        self.tools_view(id).await
    }
}
