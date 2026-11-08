//! Typy protokołu MCP (Model Context Protocol) w wersji **2025-06-18** — podzbiór potrzebny
//! klientowi (narzędzia) i serwerowi Alfy v0. Pola nieznane są ignorowane (tolerancja wersji).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wersja protokołu, którą Alfa proponuje i w której odpowiada.
pub const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";

/// Wersje akceptowane od drugiej strony (starsze mają ten sam podzbiór `tools/*`).
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// Metody i powiadomienia MCP używane przez Alfę.
pub mod methods {
    /// Inicjalizacja sesji.
    pub const INITIALIZE: &str = "initialize";
    /// Powiadomienie klienta po inicjalizacji.
    pub const INITIALIZED: &str = "notifications/initialized";
    /// Ping (obie strony).
    pub const PING: &str = "ping";
    /// Lista narzędzi.
    pub const TOOLS_LIST: &str = "tools/list";
    /// Wywołanie narzędzia.
    pub const TOOLS_CALL: &str = "tools/call";
    /// Serwer zmienił listę narzędzi.
    pub const TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";
    /// Log serwera.
    pub const MESSAGE: &str = "notifications/message";
    /// Anulowanie żądania.
    pub const CANCELLED: &str = "notifications/cancelled";
}

/// Czy wersja protokołu jest obsługiwana.
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_PROTOCOL_VERSIONS.contains(&version)
}

/// Opis implementacji (klienta lub serwera).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Implementation {
    /// Nazwa programowa.
    pub name: String,
    /// Nazwa wyświetlana.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Wersja.
    pub version: String,
}

/// Parametry `initialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    /// Wersja protokołu proponowana przez klienta.
    pub protocol_version: String,
    /// Możliwości klienta (Alfa nie deklaruje `sampling`, `roots` ani `elicitation`).
    #[serde(default)]
    pub capabilities: Value,
    /// Klient.
    pub client_info: Implementation,
}

/// Możliwość `tools` serwera.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    /// Czy serwer wysyła `notifications/tools/list_changed`.
    #[serde(default)]
    pub list_changed: bool,
}

/// Możliwości serwera (podzbiór).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ServerCapabilities {
    /// Narzędzia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    /// Logowanie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<Value>,
}

/// Wynik `initialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Wersja wybrana przez serwer.
    pub protocol_version: String,
    /// Możliwości serwera.
    #[serde(default)]
    pub capabilities: ServerCapabilities,
    /// Serwer.
    pub server_info: Implementation,
    /// Instrukcje serwera — **treść niezaufana** (nie trafia do promptu systemowego bez oznaczenia).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// Definicja narzędzia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// Nazwa (identyfikator).
    pub name: String,
    /// Nazwa wyświetlana.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Opis — **treść niezaufana**; hash opisu wiąże zgodę użytkownika (PLAN §8.7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema argumentów.
    pub input_schema: Value,
    /// JSON Schema wyniku strukturalnego.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    /// Adnotacje (wskazówki, niezaufane).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
}

/// Wynik `tools/list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    /// Narzędzia.
    pub tools: Vec<Tool>,
    /// Kursor następnej strony.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Parametry `tools/call`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CallToolParams {
    /// Nazwa narzędzia.
    pub name: String,
    /// Argumenty (obiekt).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

/// Blok treści wyniku narzędzia. Nieznane rodzaje są zachowywane jako `Unsupported`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    /// Tekst.
    Text {
        /// Treść.
        text: String,
    },
    /// Obraz (base64).
    Image {
        /// Dane base64.
        data: String,
        /// Typ MIME.
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    /// Rodzaj nieobsługiwany przez Alfę v0 (audio, zasoby…).
    #[serde(other)]
    Unsupported,
}

impl Content {
    /// Blok tekstowy.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }
}

/// Wynik `tools/call`. Błąd wykonania narzędzia = `is_error: true` (nie błąd protokołu).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// Treść.
    #[serde(default)]
    pub content: Vec<Content>,
    /// Czy wykonanie się nie powiodło.
    #[serde(default)]
    pub is_error: bool,
    /// Wynik strukturalny.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
}

impl CallToolResult {
    /// Udany wynik tekstowy.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![Content::text(text)],
            is_error: false,
            structured_content: None,
        }
    }

    /// Udany wynik z danymi strukturalnymi (tekst = ta sama wartość jako JSON, dla starszych klientów).
    pub fn structured(value: Value) -> Self {
        Self {
            content: vec![Content::text(value.to_string())],
            is_error: false,
            structured_content: Some(value),
        }
    }

    /// Błąd wykonania narzędzia.
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            content: vec![Content::text(message)],
            is_error: true,
            structured_content: None,
        }
    }

    /// Połączony tekst bloków tekstowych.
    pub fn joined_text(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| match c {
                Content::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_uses_camel_case_and_ignores_unknown_fields() {
        let tool: Tool = serde_json::from_value(json!({
            "name": "t", "description": "d", "inputSchema": {"type": "object"}, "x-extra": 1
        }))
        .unwrap();
        assert_eq!(tool.description.as_deref(), Some("d"));
        let back = serde_json::to_value(&tool).unwrap();
        assert!(back.get("inputSchema").is_some());
        assert!(back.get("title").is_none());
    }

    #[test]
    fn content_keeps_unknown_kinds() {
        let r: CallToolResult = serde_json::from_value(json!({
            "content": [{"type": "text", "text": "a"}, {"type": "audio", "data": "x", "mimeType": "audio/wav"}],
            "isError": false
        }))
        .unwrap();
        assert_eq!(r.content[1], Content::Unsupported);
        assert_eq!(r.joined_text(), "a");
        assert!(CallToolResult::failure("x").is_error);
        let s = CallToolResult::structured(json!({"a": 1}));
        assert_eq!(s.joined_text(), r#"{"a":1}"#);
    }

    #[test]
    fn versions() {
        assert!(is_supported_version(LATEST_PROTOCOL_VERSION));
        assert!(!is_supported_version("1999-01-01"));
    }
}
