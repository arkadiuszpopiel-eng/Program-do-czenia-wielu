//! Komendy serwerów MCP użytkownika (`mcp_*`, Ustawienia → Serwery MCP) — delegują do
//! `app-mcp::McpHub`. Zgody na narzędzia daje tylko użytkownik; agentka ich nie zmienia.

use crate::core::AppCore;
use crate::dto::{McpServerInput, McpServerTools, McpServerView};
use crate::error::AppError;

impl AppCore {
    /// `mcp_list`: serwery z ostatnim stanem.
    pub async fn mcp_list(&self) -> Result<Vec<McpServerView>, AppError> {
        Ok(self.inner.mcp.list().await)
    }

    /// `mcp_save`: dodanie albo zmiana serwera (wartości tajne do Menedżera poświadczeń).
    pub async fn mcp_save(&self, input: McpServerInput) -> Result<McpServerView, AppError> {
        self.inner.mcp.save(input).await
    }

    /// `mcp_remove`: usunięcie serwera, zgód i wartości tajnych.
    pub async fn mcp_remove(&self, id: String) -> Result<(), AppError> {
        self.inner.mcp.remove(&id).await
    }

    /// `mcp_tools`: uruchamia serwer (gdy włączony) i zwraca narzędzia ze stanem zgody.
    pub async fn mcp_tools(&self, id: String) -> Result<McpServerTools, AppError> {
        self.inner.mcp.tools_ipc(&id).await
    }

    /// `mcp_approve`: zgoda na narzędzie w wersji opisu o podanym odcisku.
    pub async fn mcp_approve(
        &self,
        id: String,
        tool: String,
        fingerprint: String,
    ) -> Result<McpServerTools, AppError> {
        self.inner.mcp.approve_ipc(&id, &tool, &fingerprint).await
    }

    /// `mcp_revoke`: cofnięcie zgody na narzędzie.
    pub async fn mcp_revoke(&self, id: String, tool: String) -> Result<McpServerTools, AppError> {
        self.inner.mcp.revoke_ipc(&id, &tool).await
    }
}
