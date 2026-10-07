//! Klient MCP: konfiguracja serwera zewnętrznego i trait [`McpClient`].

use std::collections::BTreeMap;
use std::path::PathBuf;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::McpError;
use crate::fingerprint::ToolFingerprint;
use crate::injection::InjectionSignal;
use crate::protocol::{CallToolResult, Implementation};
use crate::trust::{ConsentOrigin, ToolAssessment, TrustLevel};

/// Domyślny limit czasu żądania (ms).
pub const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 30_000;

/// Konfiguracja serwera MCP uruchamianego przez stdio (jedyny transport klienta w v0 —
/// żadnych połączeń sieciowych do serwerów MCP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpServerConfig {
    /// Identyfikator (kebab-case), klucz zgód na narzędzia.
    pub id: String,
    /// Program serwera.
    pub command: PathBuf,
    /// Argumenty.
    #[serde(default)]
    pub args: Vec<String>,
    /// Dodatkowe zmienne środowiskowe podane jawnie przez użytkownika (środowisko Alfy
    /// **nie** jest dziedziczone — tylko lista dozwolona + te wpisy).
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Katalog roboczy.
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    /// Poziom zaufania.
    #[serde(default)]
    pub trust: TrustLevel,
    /// Limit czasu żądania w ms.
    #[serde(default = "default_timeout")]
    pub request_timeout_ms: u64,
}

fn default_timeout() -> u64 {
    DEFAULT_REQUEST_TIMEOUT_MS
}

impl McpServerConfig {
    /// Sprawdza identyfikator (kebab-case, ≤ 64 znaki).
    pub fn validate(&self) -> Result<(), McpError> {
        let ok = !self.id.is_empty()
            && self.id.len() <= 64
            && self.id.split('-').all(|seg| {
                !seg.is_empty()
                    && seg
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            });
        if !ok {
            return Err(McpError::InvalidConfig(format!(
                "identyfikator serwera `{}` musi być w kebab-case",
                self.id
            )));
        }
        if self.request_timeout_ms == 0 {
            return Err(McpError::InvalidConfig("limit czasu = 0".into()));
        }
        Ok(())
    }
}

/// Ostrzeżenie klienta (trafia do UI i dziennika; nie zawiera argumentów narzędzi).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "warning", rename_all = "snake_case")]
pub enum McpWarning {
    /// Definicja narzędzia zmieniła się po zatwierdzeniu — narzędzie zablokowane.
    ToolDefinitionChanged {
        /// Narzędzie.
        tool: String,
        /// Zatwierdzony odcisk.
        approved: ToolFingerprint,
        /// Bieżący odcisk.
        current: ToolFingerprint,
    },
    /// Opis narzędzia zawiera sygnały injection — oznaczone jako niezaufane.
    SuspiciousTool {
        /// Narzędzie.
        tool: String,
        /// Sygnały.
        signals: Vec<InjectionSignal>,
    },
    /// Serwer ogłosił zmianę listy narzędzi.
    ToolListChanged,
    /// Serwer wysłał żądanie, którego Alfa nie obsługuje (np. `sampling/createMessage`) — odrzucone.
    RejectedServerRequest {
        /// Metoda.
        method: String,
    },
    /// Linia spoza protokołu na stdout serwera (zignorowana).
    MalformedLine {
        /// Długość linii w bajtach.
        bytes: usize,
    },
    /// Log serwera (`notifications/message`) — treść niezaufana.
    ServerLog {
        /// Poziom.
        level: String,
        /// Dane (skrócone).
        data: Value,
    },
}

/// Klient jednego serwera MCP.
#[async_trait]
pub trait McpClient: Send + Sync {
    /// Identyfikator serwera z konfiguracji.
    fn server_id(&self) -> &str;

    /// Informacje serwera z `initialize` (niezaufane).
    fn server_info(&self) -> Option<Implementation>;

    /// Wynegocjowana wersja protokołu.
    fn protocol_version(&self) -> Option<String>;

    /// Narzędzia z oceną (odcisk, stan zgody, sygnały). Odświeża listę, jeśli serwer zgłosił zmianę.
    async fn tools(&self) -> Result<Vec<ToolAssessment>, McpError>;

    /// Zgoda na narzędzie w konkretnej wersji definicji (tylko użytkownik).
    async fn consent_tool(
        &self,
        tool: &str,
        fingerprint: &ToolFingerprint,
        origin: ConsentOrigin,
    ) -> Result<(), McpError>;

    /// Wywołuje narzędzie — tylko z aktualną zgodą; przed wywołaniem ponownie sprawdza definicję,
    /// jeśli serwer zgłosił zmianę listy.
    async fn call_tool(&self, tool: &str, arguments: Value) -> Result<CallToolResult, McpError>;

    /// Ostrzeżenia zebrane od ostatniego wywołania (opróżnia bufor).
    fn take_warnings(&self) -> Vec<McpWarning>;

    /// Zamyka połączenie i kończy proces serwera.
    async fn shutdown(&self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_validation() {
        let mut cfg = McpServerConfig {
            id: "github-tools".into(),
            command: PathBuf::from("srv"),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
            trust: TrustLevel::default(),
            request_timeout_ms: default_timeout(),
        };
        assert!(cfg.validate().is_ok());
        cfg.id = "Złe ID".into();
        assert!(cfg.validate().is_err());
        cfg.id = "ok".into();
        cfg.request_timeout_ms = 0;
        assert!(cfg.validate().is_err());
        let parsed: McpServerConfig =
            serde_json::from_value(serde_json::json!({"id": "a", "command": "x"})).unwrap();
        assert_eq!(parsed.trust, TrustLevel::Untrusted);
        assert_eq!(parsed.request_timeout_ms, DEFAULT_REQUEST_TIMEOUT_MS);
    }
}
