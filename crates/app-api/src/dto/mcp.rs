//! DTO serwerów MCP podłączanych przez użytkownika (Ustawienia → Serwery MCP) — odpowiedniki
//! `types-mcp.ts`. Wartości zmiennych tajnych nigdy nie wracają do interfejsu (tylko nazwy);
//! opisy narzędzi pochodzą od serwera i są treścią niezaufaną.

use serde::{Deserialize, Serialize};

/// Poziom zaufania serwera (ustawia wyłącznie użytkownik).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum McpTrust {
    /// Każde narzędzie wymaga zgody.
    #[default]
    Untrusted,
    /// Przejrzany — nadal każde narzędzie wymaga zgody.
    Reviewed,
    /// Zaufany — nowe narzędzia bez sygnałów wstrzyknięcia zatwierdzane automatycznie.
    Trusted,
}

/// Stan połączenia z serwerem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpConnState {
    /// Wyłączony przez użytkownika.
    Disabled,
    /// Włączony, jeszcze nie uruchomiony (start przy pierwszym użyciu).
    Idle,
    /// Połączony.
    Connected,
    /// Ostatnia próba nieudana (`error`).
    Failed,
}

/// Zmienna środowiskowa serwera (wartość jawna albo tajna — wtedy tylko nazwa).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpEnvVar {
    pub name: String,
    /// Wartość jawna; `None` dla tajnej (w Menedżerze poświadczeń Windows).
    pub value: Option<String>,
    pub secret: bool,
}

/// Serwer MCP na liście.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerView {
    pub id: String,
    pub name: String,
    /// Polecenie tak, jak wpisał je użytkownik (`npx`, `uvx`, pełna ścieżka).
    pub command: String,
    /// Pełna ścieżka znalezionego programu; `None` — nie znaleziono w PATH.
    pub resolved: Option<String>,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Vec<McpEnvVar>,
    pub trust: McpTrust,
    pub enabled: bool,
    pub state: McpConnState,
    /// Czytelny powód ostatniego niepowodzenia.
    pub error: Option<String>,
    /// Nazwa i wersja z `initialize` (niezaufane).
    pub server_info: Option<String>,
    /// Liczba narzędzi z ostatniej listy i ile z nich ma zgodę.
    pub tools_total: u32,
    pub tools_approved: u32,
    pub timeout_ms: u64,
}

/// Dane formularza serwera (dodanie albo zmiana).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerInput {
    /// Pusty — nowy serwer (identyfikator z nazwy).
    pub id: Option<String>,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    /// Zmienne; tajna z `value: None` zachowuje zapisaną wartość.
    #[serde(default)]
    pub env: Vec<McpEnvVar>,
    #[serde(default)]
    pub trust: McpTrust,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

fn yes() -> bool {
    true
}

/// Stan zgody na narzędzie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpToolState {
    /// Zgoda użytkownika — agentka może wywołać.
    Approved,
    /// Zgoda automatyczna (serwer zaufany, opis bez sygnałów).
    AutoApproved,
    /// Nowe — czeka na zgodę.
    New,
    /// Opis zmienił się po zgodzie — zablokowane do ponownej zgody.
    Changed,
}

/// Narzędzie serwera MCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpToolView {
    pub name: String,
    pub title: Option<String>,
    /// Opis od serwera (niezaufany, skrócony).
    pub description: Option<String>,
    pub state: McpToolState,
    /// Odcisk bieżącej definicji (podawany przy zgodzie).
    pub fingerprint: String,
    /// Opis zawiera sygnały wstrzyknięcia poleceń.
    pub suspicious: bool,
    /// Nazwy argumentów ze schematu (podgląd).
    pub params: Vec<String>,
}

/// Wynik sprawdzenia serwera: stan po próbie i lista narzędzi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerTools {
    pub server: McpServerView,
    pub tools: Vec<McpToolView>,
}
