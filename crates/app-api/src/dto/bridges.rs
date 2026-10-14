//! DTO kart zgodności mostów CLI (Ustawienia → Mosty CLI, PLAN §1.3, §5.5,
//! §8.5) — odpowiedniki `types-bridges.ts`. Logowanie do CLI wykonuje wyłącznie użytkownik
//! (terminal z poleceniem do skopiowania); Alfa nie czyta ani nie przechowuje tokenów CLI.

use serde::{Deserialize, Serialize};

use super::hub::ComplianceStatus;

/// Źródło (regulamin, dokumentacja) z rejestru zgodności.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BridgeSource {
    pub url: String,
    pub quote: String,
}

/// Karta zgodności trasy mostu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BridgeCard {
    pub route_id: String,
    /// Most obsługiwany przez Alfę (`claude_code`, `codex`); `null` — trasa tylko w rejestrze.
    pub bridge: Option<String>,
    pub name: String,
    pub provider: String,
    /// `cli-p`, `sdk`, `api`.
    pub mode: String,
    pub program: Option<String>,
    pub detected: bool,
    pub path: Option<String>,
    /// Wersja wykryta (`--version`).
    pub version: Option<String>,
    /// Wersje przypięte w konfiguracji (`[agent_backends.bridges.<most>] pinned_versions`).
    pub pinned: Vec<String>,
    /// Przypięcie z rejestru zgodności (`cli_pinned_version`).
    pub registry_pin: Option<String>,
    /// Wykryta wersja jest przypięta (inaczej most odmówi startu).
    pub version_ok: bool,
    pub status: ComplianceStatus,
    /// Wpis rejestru nieświeży — trasa zdegradowana do szarej.
    pub stale: bool,
    /// Data weryfikacji (`YYYY-MM-DD`).
    pub verified_at: Option<String>,
    pub sources: Vec<BridgeSource>,
    pub allowed: Vec<String>,
    pub forbidden: Vec<String>,
    pub enabled: bool,
    /// Trasa zabroniona albo nieobsługiwana — włącznik nieaktywny.
    pub can_enable: bool,
    /// Jawna zgoda na uruchamianie z harmonogramu: limit na dobę (0 = brak zgody).
    pub schedule_per_day: u32,
    /// Polecenie logowania do wpisania przez użytkownika w terminalu.
    pub login_command: Option<String>,
    /// Ostatnie sprawdzenie konta (`bridges_check`); `None` — jeszcze nie sprawdzano.
    #[serde(default)]
    pub account: Option<BridgeAccount>,
}

/// Stan konta mostu CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BridgeAccountState {
    /// Zalogowano — Alfa może zlecać zadania (gdy most jest włączony).
    LoggedIn,
    /// Narzędzie jest, ale nikt się nie zalogował.
    LoggedOut,
    /// Narzędzie nie podaje stanu bez wywołania modelu (Antigravity).
    Unknown,
    /// Narzędzia nie ma na komputerze.
    NotInstalled,
    /// Polecenie stanu się nie powiodło.
    Error,
}

/// Wynik sprawdzenia konta mostu — tylko stan, sposób logowania i plan (bez e-maila, tokenów).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BridgeAccount {
    pub state: BridgeAccountState,
    /// Plan podany przez narzędzie (np. „Max”).
    pub plan: Option<String>,
    /// Sposób logowania (np. „konto claude.ai”).
    pub method: Option<String>,
    /// Zdanie dla użytkownika: co działa albo co zrobić.
    pub detail: String,
    /// Kiedy sprawdzono.
    pub checked_at: super::Iso8601,
}

/// Wynik „Zaloguj w terminalu": terminal otwarty w katalogu domowym, polecenie do skopiowania
/// (Alfa go nie wykonuje).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BridgeLogin {
    pub command: String,
    pub cwd: String,
    pub opened: bool,
}
