//! Argumenty (zamknięte schematy) i wyniki narzędzi `tools-system`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `system_processes`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProcessesArgs {
    /// Fragment nazwy obrazu (bez rozróżniania wielkości liter).
    #[serde(default)]
    pub name_contains: Option<String>,
    /// Limit wyników (1–500).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// `system_process_info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProcessInfoArgs {
    /// PID z `system_processes`.
    pub pid: u32,
}

/// `system_process_kill`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProcessKillArgs {
    /// PID z `system_processes`.
    pub pid: u32,
    /// Nazwa obrazu z listy (np. `notepad.exe`) — chroni przed zakończeniem innego procesu po
    /// ponownym użyciu PID-u.
    pub name: String,
}

/// Filtr stanu usług.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServiceFilter {
    /// Działające.
    Running,
    /// Zatrzymane.
    Stopped,
}

/// `system_services`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServicesArgs {
    /// Fragment nazwy albo nazwy wyświetlanej.
    #[serde(default)]
    pub name_contains: Option<String>,
    /// Filtr stanu.
    #[serde(default)]
    pub state: Option<ServiceFilter>,
    /// Limit wyników (1–500).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Czynność na usłudze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServiceActionArg {
    /// Start.
    Start,
    /// Zatrzymanie.
    Stop,
    /// Restart.
    Restart,
}

/// `system_service_control`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServiceControlArgs {
    /// Nazwa usługi (klucz, np. `Spooler`).
    pub name: String,
    /// Czynność.
    pub action: ServiceActionArg,
}

/// Dziennik.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogArg {
    /// Aplikacja.
    Application,
    /// System.
    System,
}

/// Najmniej ważny poziom zdarzeń.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LevelArg {
    /// Tylko krytyczne.
    Critical,
    /// Krytyczne i błędy.
    Error,
    /// Także ostrzeżenia.
    Warning,
    /// Także informacje.
    Information,
}

/// `system_events`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventsArgs {
    /// Dziennik (`application` albo `system`).
    pub log: LogArg,
    /// Najmniej ważny poziom (domyślnie `warning`).
    #[serde(default)]
    pub level: Option<LevelArg>,
    /// Dostawca (źródło), np. `Service Control Manager`.
    #[serde(default)]
    pub provider: Option<String>,
    /// Okno czasu w godzinach (1–720, domyślnie 24).
    #[serde(default)]
    pub since_hours: Option<u32>,
    /// Najwięcej zdarzeń (1–200).
    #[serde(default)]
    pub max: Option<u32>,
}

/// Zakres zmiennych.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnvScopeArg {
    /// Środowisko procesu Alfy.
    Process,
    /// Zmienne użytkownika.
    User,
    /// Zmienne systemowe.
    Machine,
}

/// `system_env`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvArgs {
    /// Zakres (domyślnie `user`).
    #[serde(default)]
    pub scope: Option<EnvScopeArg>,
    /// Fragment nazwy.
    #[serde(default)]
    pub name_contains: Option<String>,
}

/// `system_env_set` (tylko zmienne użytkownika).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvSetArgs {
    /// Nazwa zmiennej.
    pub name: String,
    /// Nowa wartość; brak = usunięcie zmiennej.
    #[serde(default)]
    pub value: Option<String>,
}

/// `system_status`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusArgs {}

/// Proces w wyniku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProcOut {
    /// PID.
    pub pid: u32,
    /// PID rodzica.
    pub parent_pid: u32,
    /// Nazwa obrazu (niezaufana).
    pub name: String,
    /// Proces bieżącego użytkownika.
    pub own: Option<bool>,
    /// Chroniony (Alfa, Broker, krytyczny) — nie da się go zakończyć.
    pub protected: bool,
}

/// Wynik `system_processes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProcessesOut {
    /// Procesy.
    pub processes: Vec<ProcOut>,
    /// Wszystkich pasujących.
    pub total: u32,
    /// Obcięto limitem.
    pub truncated: bool,
}

/// Wynik `system_process_info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProcDetailsOut {
    /// Proces.
    pub process: ProcOut,
    /// Ścieżka obrazu (dla procesów chronionych — brak).
    pub path: Option<String>,
    /// Podniesiony (administrator).
    pub elevated: Option<bool>,
    /// Sesja Windows.
    pub session_id: Option<u32>,
    /// Start (ms od epoki Unix).
    pub started_ms: Option<u64>,
    /// Wątki.
    pub threads: u32,
    /// Pamięć (KiB).
    pub memory_kb: Option<u64>,
}

/// Wynik `system_process_kill`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KillOut {
    /// PID.
    pub pid: u32,
    /// Nazwa obrazu.
    pub name: String,
}

/// Usługa w wyniku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServiceOut {
    /// Nazwa (klucz).
    pub name: String,
    /// Nazwa wyświetlana (niezaufana).
    pub display_name: String,
    /// Stan (`running`, `stopped`, …).
    pub state: String,
    /// PID procesu usługi.
    pub pid: Option<u32>,
}

/// Wynik `system_services`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServicesOut {
    /// Usługi.
    pub services: Vec<ServiceOut>,
    /// Wszystkich pasujących.
    pub total: u32,
    /// Obcięto limitem.
    pub truncated: bool,
}

/// Zdarzenie w wyniku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EventOut {
    /// Czas (ms od epoki Unix).
    pub time_ms: u64,
    /// Poziom.
    pub level: String,
    /// Dostawca.
    pub provider: String,
    /// Identyfikator.
    pub event_id: u32,
    /// Komunikat (niezaufany, zredagowany, obcięty).
    pub message: String,
}

/// Wynik `system_events`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EventsOut {
    /// Zdarzenia (najnowsze pierwsze).
    pub events: Vec<EventOut>,
    /// Mogą istnieć starsze (osiągnięto limit).
    pub truncated: bool,
}

/// Zmienna w wyniku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnvOut {
    /// Nazwa.
    pub name: String,
    /// Wartość (zredagowana; brak, gdy ukryta).
    pub value: Option<String>,
    /// Ukryta (nazwa sekretu).
    pub hidden: bool,
}

/// Wynik `system_env`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnvListOut {
    /// Zakres.
    pub scope: String,
    /// Zmienne.
    pub vars: Vec<EnvOut>,
}

/// Wynik `system_env_set`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnvSetOut {
    /// Nazwa.
    pub name: String,
    /// Usunięto zmienną.
    pub deleted: bool,
    /// Zmienna istniała wcześniej.
    pub previous_set: bool,
    /// Identyfikator cofnięcia (karta „Cofnij”).
    pub undo_id: u64,
}

/// Zasilanie w wyniku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PowerOut {
    /// Źródło (`ac`, `battery`, `unknown`).
    pub source: String,
    /// Bateria obecna.
    pub battery_present: bool,
    /// Poziom baterii.
    pub battery_percent: Option<u8>,
    /// Oszczędzanie energii.
    pub saver: bool,
}

/// Monitor w wyniku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DisplayOut {
    /// Indeks.
    pub index: u32,
    /// Szerokość (px).
    pub width: i32,
    /// Wysokość (px).
    pub height: i32,
    /// DPI.
    pub dpi: u32,
    /// Główny.
    pub primary: bool,
}

/// Urządzenie audio w wyniku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AudioOut {
    /// Nazwa (niezaufana).
    pub name: String,
    /// Kierunek (`capture`, `render`).
    pub direction: String,
}

/// Wynik `system_status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusOut {
    /// Zasilanie.
    pub power: Option<PowerOut>,
    /// Monitory.
    pub displays: Vec<DisplayOut>,
    /// Urządzenia audio.
    pub audio: Vec<AudioOut>,
    /// Czego nie udało się odczytać.
    pub unavailable: Vec<String>,
}
