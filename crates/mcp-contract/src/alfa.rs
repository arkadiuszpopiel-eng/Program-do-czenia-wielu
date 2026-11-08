//! Serwer MCP Alfy (PLAN §9.7, §8.5): v0 (F4) — schowek i okna + wewnętrzne `approve` dla mostu
//! Claude Code; v1 (F6) — UI Automation (drzewo, wyszukiwanie, tekst, akcje przez wzorce),
//! zrzut ekranu z maskowaniem i rejestr **tylko do odczytu** (definicje: `alfa_v1`). Żadnych
//! narzędzi fs/shell (most ma je natywnie w worktree, §8.5). Każde wywołanie v1 idzie przez
//! Brokera z podmiotem „most CLI”, a wynik jest oznaczony `unverified_by_alfa`.
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
    /// v1: drzewo UI Automation okna (bez okien chronionych, hasła bez wartości).
    UiaTree,
    /// v1: wyszukiwanie elementów UI.
    UiaFind,
    /// v1: tekst elementu (`TextPattern`, tylko odczyt).
    UiaReadText,
    /// v1: akcja przez wzorzec UI Automation.
    UiaAct,
    /// v1: zrzut ekranu/okna z maskowaniem.
    ScreenCapture,
    /// v1: rejestr `HKCU`/`HKLM` tylko do odczytu (deny-lista kluczy z sekretami).
    RegistryRead,
}

/// Pole wyniku narzędzi v1: wynik pochodzi z narzędzia Alfy, ale użycie go przez most (opaque
/// worker) nie jest weryfikowane przez Alfę (PLAN §8.5).
pub const UNVERIFIED_FIELD: &str = "unverified_by_alfa";

impl AlfaTool {
    /// Narzędzia Windows v0 (bez wewnętrznego `approve`).
    pub const WINDOWS_V0: [AlfaTool; 4] = [
        AlfaTool::ClipboardRead,
        AlfaTool::ClipboardWrite,
        AlfaTool::WindowsList,
        AlfaTool::WindowsFocus,
    ];

    /// Narzędzia dodane w v1 (F6).
    pub const V1_ONLY: [AlfaTool; 6] = [
        AlfaTool::UiaTree,
        AlfaTool::UiaFind,
        AlfaTool::UiaReadText,
        AlfaTool::UiaAct,
        AlfaTool::ScreenCapture,
        AlfaTool::RegistryRead,
    ];

    /// Narzędzia Windows v1 (v0 + UIA, zrzut, rejestr; bez wewnętrznego `approve`).
    pub const WINDOWS_V1: [AlfaTool; 10] = [
        AlfaTool::ClipboardRead,
        AlfaTool::ClipboardWrite,
        AlfaTool::WindowsList,
        AlfaTool::WindowsFocus,
        AlfaTool::UiaTree,
        AlfaTool::UiaFind,
        AlfaTool::UiaReadText,
        AlfaTool::UiaAct,
        AlfaTool::ScreenCapture,
        AlfaTool::RegistryRead,
    ];

    /// Wszystkie narzędzia.
    pub const ALL: [AlfaTool; 11] = [
        AlfaTool::ClipboardRead,
        AlfaTool::ClipboardWrite,
        AlfaTool::WindowsList,
        AlfaTool::WindowsFocus,
        AlfaTool::Approve,
        AlfaTool::UiaTree,
        AlfaTool::UiaFind,
        AlfaTool::UiaReadText,
        AlfaTool::UiaAct,
        AlfaTool::ScreenCapture,
        AlfaTool::RegistryRead,
    ];

    /// Czy narzędzie należy do v1 (przez Brokera, wynik `unverified_by_alfa`).
    pub fn is_v1(self) -> bool {
        Self::V1_ONLY.contains(&self)
    }

    /// Nazwa na drucie MCP.
    pub fn name(self) -> &'static str {
        match self {
            AlfaTool::ClipboardRead => "clipboard_read",
            AlfaTool::ClipboardWrite => "clipboard_write",
            AlfaTool::WindowsList => "windows_list",
            AlfaTool::WindowsFocus => "windows_focus",
            AlfaTool::Approve => "approve",
            AlfaTool::UiaTree => "uia_tree",
            AlfaTool::UiaFind => "uia_find",
            AlfaTool::UiaReadText => "uia_read_text",
            AlfaTool::UiaAct => "uia_act",
            AlfaTool::ScreenCapture => "screen_capture",
            AlfaTool::RegistryRead => "registry_read",
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
            AlfaTool::UiaTree
            | AlfaTool::UiaFind
            | AlfaTool::UiaReadText
            | AlfaTool::UiaAct
            | AlfaTool::ScreenCapture => "gui.control",
            // Broker: `system.admin(other: reg query …)` — najostrzejsza klasa do czasu
            // osobnej zdolności odczytu rejestru (SPEC mcp, otwarte pytania).
            AlfaTool::RegistryRead => "registry.read",
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
            AlfaTool::UiaTree
            | AlfaTool::UiaFind
            | AlfaTool::UiaReadText
            | AlfaTool::UiaAct
            | AlfaTool::ScreenCapture
            | AlfaTool::RegistryRead => return crate::alfa_v1::definition(self),
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
            for banned in [
                "fs",
                "file",
                "shell",
                "exec",
                "cmd",
                "process",
                "write_reg",
                "set_reg",
            ] {
                assert!(!n.contains(banned), "{n} zawiera {banned}");
            }
            // Rejestr wyłącznie do odczytu (PLAN §8.5: F6 — UIA, zrzuty, rejestr).
            if n.contains("registry") {
                assert_eq!(n, "registry_read");
                let def = tool.definition();
                let ann = def.annotations.unwrap_or_default();
                assert_eq!(ann["readOnlyHint"], true);
            }
        }
        assert!(AlfaTool::V1_ONLY.iter().all(|t| t.is_v1()));
        assert!(AlfaTool::WINDOWS_V0.iter().all(|t| !t.is_v1()));
        assert!(!AlfaTool::WINDOWS_V1.contains(&AlfaTool::Approve));
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
