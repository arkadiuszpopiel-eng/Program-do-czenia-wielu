//! Prośby o uprawnienia mostu i port kanału zatwierdzeń [`ApprovalSink`] (Broker-UI podepnie się
//! w F3/F4). Natywne „ask” CLI są przekierowane tutaj: Claude Code — `--permission-prompt-tool`
//! (narzędzie MCP Alfy `approve`), Codex — żądania zatwierdzeń `app-server`.

use std::fmt;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::task::{BridgeKind, TaskId};

/// Domyślny limit oczekiwania na decyzję (ms); po nim — odmowa.
pub const DEFAULT_APPROVAL_TIMEOUT_MS: u64 = 10 * 60 * 1000;

/// Identyfikator prośby (unikalny w backendzie).
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct PermissionRequestId(pub String);

impl fmt::Display for PermissionRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Rodzaj prośby.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKind {
    /// Użycie narzędzia (Claude Code: dowolne narzędzie spoza `--allowedTools`).
    Tool,
    /// Wykonanie polecenia (Codex `commandExecution`).
    Command,
    /// Zmiana plików (Codex `fileChange`).
    FileChange,
}

/// Prośba o uprawnienie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PermissionRequest {
    /// Identyfikator.
    pub id: PermissionRequestId,
    /// Zadanie.
    pub task: TaskId,
    /// Most.
    pub bridge: BridgeKind,
    /// Rodzaj.
    pub kind: PermissionKind,
    /// Narzędzie / polecenie (np. `Bash`, `Write`, `commandExecution`).
    pub tool: String,
    /// Argumenty (niezaufane; UI pokazuje je jako dane, nie instrukcje).
    pub input: Value,
    /// Uzasadnienie podane przez CLI (niezaufane).
    #[serde(default)]
    pub reason: Option<String>,
    /// Identyfikator wywołania w CLI.
    #[serde(default)]
    pub call_id: Option<String>,
}

/// Decyzja.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Zgoda; opcjonalnie zmienione argumenty (tylko Claude Code).
    Allow {
        /// Zmienione argumenty.
        #[serde(default)]
        updated_input: Option<Value>,
    },
    /// Odmowa z komunikatem dla agenta.
    Deny {
        /// Komunikat.
        message: String,
    },
}

impl ApprovalDecision {
    /// Zgoda bez zmian.
    pub fn allow() -> Self {
        Self::Allow {
            updated_input: None,
        }
    }

    /// Odmowa.
    pub fn deny(message: impl Into<String>) -> Self {
        Self::Deny {
            message: message.into(),
        }
    }

    /// Czy to zgoda.
    pub fn is_allow(&self) -> bool {
        matches!(self, Self::Allow { .. })
    }
}

/// Kanał zatwierdzeń. Backend: (1) emituje `AgentEvent::PermissionRequest`, (2) wywołuje
/// [`ApprovalSink::request`], (3) czeka na pierwszą z decyzji: wynik `request` (jeśli `Some`)
/// albo `AgentBackend::approve`; po limicie czasu — odmowa.
#[async_trait]
pub trait ApprovalSink: Send + Sync {
    /// Przekazuje prośbę. `Some(decyzja)` — kanał zna decyzję (np. Broker-UI); `None` —
    /// decyzja przyjdzie przez `AgentBackend::approve`.
    async fn request(&self, request: PermissionRequest) -> Option<ApprovalDecision>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_serialization() {
        assert!(ApprovalDecision::allow().is_allow());
        assert!(!ApprovalDecision::deny("nie").is_allow());
        let json = serde_json::to_value(ApprovalDecision::deny("nie")).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"decision": "deny", "message": "nie"})
        );
        assert_eq!(PermissionRequestId("p-1".into()).to_string(), "p-1");
    }
}
