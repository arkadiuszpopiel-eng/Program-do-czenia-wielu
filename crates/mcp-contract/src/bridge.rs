//! Host serwera MCP Alfy dla mostów CLI: rejestracja z tokenem sesyjnym (TTL), kanał lokalny
//! (named pipe z ACL / gniazdo Unix 0600 — **nigdy TCP**), stdio-proxy i narzędzie `approve`.
//!
//! Przepływ: most rejestruje zadanie i dostaje [`McpServerLaunch`] (program `alfa-mcp-proxy`
//! ze zmiennymi `ALFA_MCP_ENDPOINT`/`ALFA_MCP_TOKEN`); CLI uruchamia proxy jako serwer MCP stdio;
//! proxy łączy się z kanałem lokalnym i wysyła [`ProxyHello`] w pierwszej linii; host sprawdza
//! token (stały czas porównania, TTL, unieważnienie) i obsługuje MCP w zakresie rejestracji.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::alfa::{ALFA_SERVER_NAME, AlfaTool};
use crate::error::McpError;

/// Zmienna z adresem kanału lokalnego dla proxy.
pub const ENV_ENDPOINT: &str = "ALFA_MCP_ENDPOINT";
/// Zmienna z tokenem sesyjnym dla proxy (nigdy nie trafia do logów).
pub const ENV_TOKEN: &str = "ALFA_MCP_TOKEN";
/// Nazwa programu proxy.
pub const PROXY_PROGRAM: &str = "alfa-mcp-proxy";
/// Limit długości linii powitania (bajty).
pub const MAX_HELLO_BYTES: usize = 4096;
/// Czas na powitanie po połączeniu (ms).
pub const HELLO_TIMEOUT_MS: u64 = 5_000;
/// Domyślny TTL tokenu (ms): 4 h — pokrywa typowe zadanie mostu; unieważniany po zakończeniu zadania.
pub const DEFAULT_TOKEN_TTL_MS: u64 = 4 * 60 * 60 * 1000;
/// Rodzaj pierwszej wiadomości proxy.
pub const HELLO_TYPE: &str = "alfa-mcp-hello";
/// Wersja powitania.
pub const HELLO_VERSION: u32 = 1;

/// Kanał lokalny hosta.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "path", rename_all = "snake_case")]
pub enum LocalEndpoint {
    /// Gniazdo Unix (testy/nie-Windows): katalog 0700, gniazdo 0600.
    UnixSocket(PathBuf),
    /// Named pipe Windows (`\\.\pipe\...`) z ACL na SID bieżącego użytkownika.
    NamedPipe(String),
}

impl LocalEndpoint {
    /// Zapis w zmiennej środowiskowej: `unix:<ścieżka>` albo `pipe:<nazwa>`.
    pub fn to_env_value(&self) -> String {
        match self {
            LocalEndpoint::UnixSocket(p) => format!("unix:{}", p.display()),
            LocalEndpoint::NamedPipe(n) => format!("pipe:{n}"),
        }
    }

    /// Odczyt ze zmiennej środowiskowej. Inne schematy (np. `tcp:`) są odrzucane.
    pub fn parse(value: &str) -> Option<Self> {
        if let Some(path) = value.strip_prefix("unix:") {
            return (!path.is_empty()).then(|| LocalEndpoint::UnixSocket(PathBuf::from(path)));
        }
        let name = value.strip_prefix("pipe:")?;
        name.starts_with(r"\\.\pipe\")
            .then(|| LocalEndpoint::NamedPipe(name.to_owned()))
    }
}

/// Token sesyjny. `Debug`/`Display` nie ujawniają wartości; porównanie w stałym czasie.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionToken(String);

impl SessionToken {
    /// Opakowuje wartość (generuje ją `-impl` z CSPRNG).
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Wartość do przekazania proxy (wyłącznie w zmiennej środowiskowej procesu potomnego).
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Porównanie w stałym czasie względem długości krótszego tekstu.
    pub fn matches(&self, presented: &str) -> bool {
        let a = self.0.as_bytes();
        let b = presented.as_bytes();
        let mut diff = u8::from(a.len() != b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0 && !a.is_empty()
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionToken(***)")
    }
}

/// Pierwsza linia proxy → host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyHello {
    /// Zawsze [`HELLO_TYPE`].
    #[serde(rename = "type")]
    pub kind: String,
    /// Wersja powitania.
    pub version: u32,
    /// Token sesyjny.
    pub token: String,
}

impl ProxyHello {
    /// Powitanie z tokenem.
    pub fn new(token: &str) -> Self {
        Self {
            kind: HELLO_TYPE.to_owned(),
            version: HELLO_VERSION,
            token: token.to_owned(),
        }
    }

    /// Linia do wysłania (bez `\n`).
    pub fn to_line(&self) -> String {
        json!({"type": self.kind, "version": self.version, "token": self.token}).to_string()
    }

    /// Parsuje linię powitania; `None` przy złym formacie, typie, wersji lub długości.
    pub fn parse(line: &str) -> Option<Self> {
        if line.len() > MAX_HELLO_BYTES {
            return None;
        }
        let hello: ProxyHello = serde_json::from_str(line.trim()).ok()?;
        (hello.kind == HELLO_TYPE && hello.version == HELLO_VERSION && !hello.token.is_empty())
            .then_some(hello)
    }
}

/// Identyfikator rejestracji (nie jest sekretem).
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct RegistrationId(pub String);

/// Zakres rejestracji: które narzędzia widzi most i czyje to zadanie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BridgeScope {
    /// Etykieta (np. id zadania) — do dziennika.
    pub label: String,
    /// Narzędzia Windows dostępne dla mostu (`approve` dochodzi, gdy podano router zatwierdzeń).
    pub tools: BTreeSet<AlfaTool>,
}

impl BridgeScope {
    /// Zakres z narzędziami Windows v0.
    pub fn windows_v0(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tools: AlfaTool::WINDOWS_V0.into_iter().collect(),
        }
    }
}

/// Jak CLI ma uruchomić serwer MCP Alfy (wpis `mcpServers.alfa`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerLaunch {
    /// Nazwa serwera (`alfa`).
    pub name: String,
    /// Program (`alfa-mcp-proxy`).
    pub command: PathBuf,
    /// Argumenty.
    pub args: Vec<String>,
    /// Zmienne środowiskowe proxy (zawiera token — plik konfiguracji zapisuje się z prawami 0600).
    pub env: BTreeMap<String, String>,
}

impl McpServerLaunch {
    /// Dokument `--mcp-config` Claude Code: `{"mcpServers": {"alfa": {"type": "stdio", ...}}}`.
    pub fn to_mcp_config(&self) -> Value {
        json!({"mcpServers": {self.name.clone(): {
            "type": "stdio",
            "command": self.command.to_string_lossy(),
            "args": self.args,
            "env": self.env,
        }}})
    }

    /// Kanał lokalny zapisany w `env`.
    pub fn endpoint(&self) -> Option<LocalEndpoint> {
        self.env
            .get(ENV_ENDPOINT)
            .and_then(|v| LocalEndpoint::parse(v))
    }

    /// Token zapisany w `env`.
    pub fn token(&self) -> Option<&str> {
        self.env.get(ENV_TOKEN).map(String::as_str)
    }
}

/// Wynik rejestracji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeRegistration {
    /// Identyfikator (do unieważnienia).
    pub id: RegistrationId,
    /// Wpis serwera dla CLI.
    pub launch: McpServerLaunch,
    /// Wygaśnięcie tokenu (ms zegara hosta).
    pub expires_at_ms: u64,
}

/// Wejście narzędzia `approve` wg `--permission-prompt-tool` Claude Code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PermissionPromptRequest {
    /// Narzędzie, o które prosi CLI (np. `Write`, `Bash`, `mcp__alfa__clipboard_read`).
    pub tool_name: String,
    /// Argumenty narzędzia.
    #[serde(default)]
    pub input: Value,
    /// Identyfikator wywołania w CLI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

/// Odpowiedź narzędzia `approve` (JSON w bloku tekstowym wyniku).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "behavior", rename_all = "lowercase")]
pub enum PermissionPromptResponse {
    /// Zgoda (argumenty mogą być zmienione przez użytkownika).
    Allow {
        /// Argumenty do użycia.
        #[serde(rename = "updatedInput")]
        updated_input: Value,
    },
    /// Odmowa z komunikatem dla modelu.
    Deny {
        /// Komunikat.
        message: String,
    },
}

/// Odbiorca próśb o uprawnienia z narzędzia `approve` (implementuje most).
#[async_trait]
pub trait ApprovalRouter: Send + Sync {
    /// Przekazuje prośbę do kanału zatwierdzeń i czeka na decyzję (odmowa po limicie czasu).
    async fn permission_prompt(&self, request: PermissionPromptRequest)
    -> PermissionPromptResponse;
}

/// Host serwera MCP Alfy dla mostów.
#[async_trait]
pub trait BridgeMcpHost: Send + Sync {
    /// Rejestruje zadanie mostu: nowy token z TTL, zakres narzędzi, opcjonalny router zatwierdzeń.
    async fn register(
        &self,
        scope: BridgeScope,
        approvals: Option<Arc<dyn ApprovalRouter>>,
    ) -> Result<BridgeRegistration, McpError>;

    /// Unieważnia token i zamyka połączenia rejestracji.
    async fn revoke(&self, id: &RegistrationId) -> Result<(), McpError>;
}

/// Nazwa serwera w konfiguracji, gdyby ktoś podał inną — zawsze [`ALFA_SERVER_NAME`].
pub fn server_name() -> &'static str {
    ALFA_SERVER_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_env_round_trip_and_no_tcp() {
        let unix = LocalEndpoint::UnixSocket(PathBuf::from("/run/alfa/mcp.sock"));
        assert_eq!(LocalEndpoint::parse(&unix.to_env_value()), Some(unix));
        let pipe = LocalEndpoint::NamedPipe(r"\\.\pipe\alfa-mcp-1".into());
        assert_eq!(LocalEndpoint::parse(&pipe.to_env_value()), Some(pipe));
        assert_eq!(LocalEndpoint::parse("tcp:localhost:9000"), None);
        assert_eq!(LocalEndpoint::parse("pipe:C:/x"), None);
        assert_eq!(LocalEndpoint::parse("unix:"), None);
    }

    #[test]
    fn token_is_redacted_and_compared_exactly() {
        let t = SessionToken::new("abc123");
        assert_eq!(format!("{t:?}"), "SessionToken(***)");
        assert!(t.matches("abc123"));
        assert!(!t.matches("abc124"));
        assert!(!t.matches("abc12"));
        assert!(!SessionToken::new("").matches(""));
    }

    #[test]
    fn hello_parsing() {
        let line = ProxyHello::new("tok").to_line();
        assert_eq!(ProxyHello::parse(&line), Some(ProxyHello::new("tok")));
        assert_eq!(
            ProxyHello::parse(r#"{"type":"x","version":1,"token":"t"}"#),
            None
        );
        assert_eq!(
            ProxyHello::parse(r#"{"type":"alfa-mcp-hello","version":2,"token":"t"}"#),
            None
        );
        assert_eq!(ProxyHello::parse(&"x".repeat(MAX_HELLO_BYTES + 1)), None);
    }

    #[test]
    fn permission_prompt_wire_format() {
        let allow = PermissionPromptResponse::Allow {
            updated_input: json!({"a": 1}),
        };
        assert_eq!(
            serde_json::to_value(&allow).unwrap(),
            json!({"behavior": "allow", "updatedInput": {"a": 1}})
        );
        let deny: PermissionPromptResponse =
            serde_json::from_value(json!({"behavior": "deny", "message": "nie"})).unwrap();
        assert_eq!(
            deny,
            PermissionPromptResponse::Deny {
                message: "nie".into()
            }
        );
        let req: PermissionPromptRequest =
            serde_json::from_value(json!({"tool_name": "Bash", "input": {"command": "ls"}}))
                .unwrap();
        assert_eq!(req.tool_use_id, None);
    }

    #[test]
    fn launch_renders_mcp_config() {
        let launch = McpServerLaunch {
            name: server_name().into(),
            command: PathBuf::from("alfa-mcp-proxy"),
            args: vec![],
            env: BTreeMap::from([
                (ENV_ENDPOINT.to_owned(), "unix:/tmp/s".to_owned()),
                (ENV_TOKEN.to_owned(), "t".to_owned()),
            ]),
        };
        let cfg = launch.to_mcp_config();
        assert_eq!(cfg["mcpServers"]["alfa"]["type"], "stdio");
        assert_eq!(launch.token(), Some("t"));
        assert!(launch.endpoint().is_some());
        assert_eq!(BridgeScope::windows_v0("x").tools.len(), 4);
    }
}
