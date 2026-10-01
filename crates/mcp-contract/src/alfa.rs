//! Serwer MCP Alfy v0 (PLAN §9.7, F4): narzędzia **tylko** schowek i okna + wewnętrzne `approve`
//! dla mostu Claude Code. Żadnych narzędzi fs/shell (most ma je natywnie w worktree, §8.5).
//!
//! Nazwy narzędzi na drucie używają `_` zamiast `.` (`clipboard_read`): API modeli (Anthropic,
//! OpenAI) akceptują w nazwach narzędzi tylko `[a-zA-Z0-9_-]`. Zdolność Brokera zachowuje
//! notację z kropką (`clipboard.read`) — patrz [`AlfaTool::capability`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::protocol::{Implementation, Tool};

/// Nazwa serwera w konfiguracji MCP mostu (`--mcp-config`).
pub const ALFA_SERVER_NAME: &str = "alfa";

/// Pełna nazwa narzędzia zatwierdzeń dla `claude --permission-prompt-tool`.
pub const PERMISSION_PROMPT_TOOL: &str = "mcp__alfa__approve";

/// Maksymalna długość tekstu zapisywanego do schowka (znaki).
pub const MAX_CLIPBOARD_WRITE_CHARS: usize = 1_000_000;

/// Narzędzie serwera Alfy.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AlfaTool {
    /// Odczyt schowka.
    ClipboardRead,
    /// Zapis tekstu do schowka (odwracalny).
    ClipboardWrite,
    /// Lista okien najwyższego poziomu (bez okien chronionych).
    WindowsList,
    /// Przeniesienie okna na pierwszy plan (nigdy okien Alfy/Brokera).
    WindowsFocus,
    /// Wewnętrzne: prośba mostu o uprawnienie → kanał zatwierdzeń.
    Approve,
}

impl AlfaTool {
    /// Narzędzia Windows v0 (bez wewnętrznego `approve`).
    pub const WINDOWS_V0: [AlfaTool; 4] = [
        AlfaTool::ClipboardRead,
        AlfaTool::ClipboardWrite,
        AlfaTool::WindowsList,
        AlfaTool::WindowsFocus,
    ];

    /// Wszystkie narzędzia v0.
    pub const ALL: [AlfaTool; 5] = [
        AlfaTool::ClipboardRead,
        AlfaTool::ClipboardWrite,
        AlfaTool::WindowsList,
        AlfaTool::WindowsFocus,
        AlfaTool::Approve,
    ];

    /// Nazwa na drucie MCP.
    pub fn name(self) -> &'static str {
        match self {
            AlfaTool::ClipboardRead => "clipboard_read",
            AlfaTool::ClipboardWrite => "clipboard_write",
            AlfaTool::WindowsList => "windows_list",
            AlfaTool::WindowsFocus => "windows_focus",
            AlfaTool::Approve => "approve",
        }
    }

    /// Zdolność (token Brokera, PLAN §8.1).
    pub fn capability(self) -> &'static str {
        match self {
            AlfaTool::ClipboardRead => "clipboard.read",
            AlfaTool::ClipboardWrite => "clipboard.write",
            AlfaTool::WindowsList => "windows.list",
            AlfaTool::WindowsFocus => "windows.focus",
            AlfaTool::Approve => "approvals.request",
        }
    }

    /// Narzędzie po nazwie z drutu.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    /// Definicja MCP (opis, schemat wejścia, adnotacje).
    pub fn definition(self) -> Tool {
        let empty = json!({"type": "object", "properties": {}, "additionalProperties": false});
        let (description, schema, annotations) = match self {
            AlfaTool::ClipboardRead => (
                "Zwraca bieżącą zawartość schowka Windows (tekst, lista plików albo opis obrazu).",
                empty,
                json!({"readOnlyHint": true, "openWorldHint": false}),
            ),
            AlfaTool::ClipboardWrite => (
                "Zapisuje tekst do schowka Windows. Poprzednia zawartość jest zachowana do cofnięcia.",
                json!({"type": "object", "properties": {"text": {"type": "string",
                    "maxLength": MAX_CLIPBOARD_WRITE_CHARS}},
                    "required": ["text"], "additionalProperties": false}),
                json!({"readOnlyHint": false, "destructiveHint": false, "openWorldHint": false}),
            ),
            AlfaTool::WindowsList => (
                "Zwraca listę widocznych okien najwyższego poziomu: identyfikator, tytuł, proces.",
                empty,
                json!({"readOnlyHint": true, "openWorldHint": false}),
            ),
            AlfaTool::WindowsFocus => (
                "Przenosi okno o podanym identyfikatorze (z windows_list) na pierwszy plan.",
                json!({"type": "object", "properties": {"id": {"type": "integer", "minimum": 0}},
                    "required": ["id"], "additionalProperties": false}),
                json!({"readOnlyHint": false, "destructiveHint": false, "openWorldHint": false}),
            ),
            AlfaTool::Approve => (
                "Wewnętrzne narzędzie zatwierdzeń mostu: przekazuje prośbę o uprawnienie do \
                 użytkownika i zwraca decyzję.",
                json!({"type": "object", "properties": {
                    "tool_name": {"type": "string"},
                    "input": {"type": "object"},
                    "tool_use_id": {"type": "string"}},
                    "required": ["tool_name", "input"]}),
                json!({"readOnlyHint": true, "openWorldHint": false}),
            ),
        };
        Tool {
            name: self.name().to_owned(),
            title: None,
            description: Some(description.to_owned()),
            input_schema: schema,
            output_schema: None,
            annotations: Some(annotations),
        }
    }
}

/// Informacje serwera Alfy w `initialize`.
pub fn alfa_server_info() -> Implementation {
    Implementation {
        name: ALFA_SERVER_NAME.to_owned(),
        title: Some("Alfa".to_owned()),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

/// Czy nazwa procesu należy do chronionych (Alfa, Broker, Broker-UI, watchdog, proxy MCP) —
/// wobec nich zakazane jest `gui.control` (AGENTS.md), więc `windows_focus` odmawia,
/// a `windows_list` ich nie pokazuje.
pub fn is_protected_process(process: &str) -> bool {
    let file = process.rsplit(['\\', '/']).next().unwrap_or(process);
    let lower = file.to_ascii_lowercase();
    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
    stem == "alfa"
        || stem.starts_with("alfa-")
        || stem.starts_with("alfa_")
        || stem.contains("safety-broker")
        || stem.contains("broker-ui")
        || stem.contains("alfa-watchdog")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonrpc::RequestId;

    #[test]
    fn names_round_trip_and_match_model_tool_regex() {
        for tool in AlfaTool::ALL {
            assert_eq!(AlfaTool::from_name(tool.name()), Some(tool));
            assert!(
                tool.name()
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            );
            let def = tool.definition();
            assert_eq!(def.name, tool.name());
            assert_eq!(def.input_schema["type"], "object");
            assert!(crate::injection::scan_tool(&def).is_empty(), "{tool:?}");
        }
        assert_eq!(AlfaTool::from_name("fs_read"), None);
        assert_eq!(AlfaTool::ClipboardRead.capability(), "clipboard.read");
        assert_eq!(
            PERMISSION_PROMPT_TOOL,
            format!("mcp__{ALFA_SERVER_NAME}__approve")
        );
        assert_eq!(alfa_server_info().name, "alfa");
        let _ = RequestId::Number(1);
    }

    #[test]
    fn no_fs_or_shell_tools() {
        for tool in AlfaTool::ALL {
            let n = tool.name();
            for banned in ["fs", "file", "shell", "exec", "cmd", "process", "registry"] {
                assert!(!n.contains(banned), "{n} zawiera {banned}");
            }
        }
    }

    #[test]
    fn protected_processes() {
        assert!(is_protected_process(r"C:\Program Files\Alfa\alfa.exe"));
        assert!(is_protected_process("Alfa-MCP-Proxy.exe"));
        assert!(is_protected_process("safety-broker.exe"));
        assert!(is_protected_process("broker-ui"));
        assert!(!is_protected_process("notepad.exe"));
        assert!(!is_protected_process("alfabet.exe"));
    }
}
