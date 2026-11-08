//! Port systemu dla `tools-system` (PLAN §7.2 „Procesy i usługi”, „System i ustawienia”):
//! procesy (lista, szczegóły, zakończenie z tożsamością), usługi (lista, start/stop), Dziennik
//! zdarzeń (`Application`/`System`, zapytanie XPath budowane tutaj z wartości sprawdzonych) i
//! zmienne środowiskowe (odczyt z ukryciem sekretów, zapis wyłącznie zmiennych użytkownika).
//!
//! Port egzekwuje niezmienniki, których nie da się obejść z wyższej warstwy: proces chroniony
//! strażnikiem celów (Alfa, jej drzewo, Broker, watchdog) albo krytyczny dla systemu nie jest
//! kończony, a zakończenie idzie przez uchwyt otwarty po PID-zie dopiero po ponownym sprawdzeniu
//! obrazu i czasu startu ([`ProcessIdentity`]). Polityki: `crate::sys_policy`.

use platform_contract::TargetGuard;
use serde::{Deserialize, Serialize};

use crate::sys_policy::protected_process;

/// Najwięcej zdarzeń w jednym zapytaniu.
pub const MAX_EVENTS: u32 = 200;
/// Najdłuższy komunikat zdarzenia (znaki) zwracany przez port.
pub const MAX_EVENT_MESSAGE_CHARS: usize = 4_000;
/// Najdłuższy czas oczekiwania na zmianę stanu usługi (ms).
pub const MAX_SERVICE_WAIT_MS: u64 = 30_000;

/// Błąd portu systemu.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SysError {
    /// Nie znaleziono (proces, usługa, zmienna).
    #[error("nie znaleziono: {0}")]
    NotFound(String),
    /// Brak uprawnień (zwykle: wymaga administratora).
    #[error("brak uprawnień: {0}")]
    PermissionDenied(String),
    /// Cel chroniony (Alfa, Broker, watchdog, proces krytyczny, inny użytkownik).
    #[error("cel chroniony: {0}")]
    Protected(String),
    /// Tożsamość celu zmieniła się od odczytu (np. PID użyty ponownie).
    #[error("cel zmienił się od odczytu: {0}")]
    Changed(String),
    /// Niepoprawne dane wejściowe.
    #[error("niepoprawne dane: {0}")]
    Invalid(String),
    /// Nieobsługiwane na tej platformie.
    #[error("nieobsługiwane: {0}")]
    Unsupported(String),
    /// Przekroczony limit czasu.
    #[error("przekroczony limit czasu: {0}")]
    Timeout(String),
    /// Błąd systemu.
    #[error("błąd systemu: {0}")]
    Io(String),
}

/// Proces na liście.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessEntry {
    /// PID.
    pub pid: u32,
    /// PID rodzica (0 = brak).
    pub parent_pid: u32,
    /// Nazwa pliku obrazu (np. `notepad.exe`).
    pub image: String,
    /// Sesja Windows (`None` = nieznana).
    pub session_id: Option<u32>,
    /// Proces bieżącego użytkownika (`None` = nie da się ustalić — traktowane jak cudzy).
    pub own: Option<bool>,
    /// Liczba wątków.
    pub threads: u32,
}

/// Szczegóły procesu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessDetails {
    /// Wpis listy.
    pub entry: ProcessEntry,
    /// Pełna ścieżka obrazu (gdy dostępna).
    pub path: Option<String>,
    /// Czas startu (ms od epoki Unix).
    pub started_ms: Option<u64>,
    /// Zestaw roboczy (KiB).
    pub memory_kb: Option<u64>,
    /// Proces podniesiony (administrator).
    pub elevated: Option<bool>,
}

/// Tożsamość procesu do zakończenia — port porównuje ją z procesem pod uchwytem tuż przed akcją.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessIdentity {
    /// PID.
    pub pid: u32,
    /// Nazwa pliku obrazu (porównanie bez rozróżniania wielkości liter).
    pub image: String,
    /// Czas startu z odczytu (`None` = nieznany; port wtedy wymaga zgodności obrazu).
    pub started_ms: Option<u64>,
}

impl ProcessIdentity {
    /// Tożsamość ze szczegółów.
    pub fn of(details: &ProcessDetails) -> Self {
        Self {
            pid: details.entry.pid,
            image: details.entry.image.clone(),
            started_ms: details.started_ms,
        }
    }

    /// Czy proces (obraz, czas startu) jest tym samym procesem.
    pub fn matches(&self, image: &str, started_ms: Option<u64>) -> bool {
        let same_image = platform_contract::image_file_name(image)
            == platform_contract::image_file_name(&self.image);
        let same_start = match (self.started_ms, started_ms) {
            (Some(a), Some(b)) => a == b,
            (None, _) => true,
            (Some(_), None) => false,
        };
        same_image && same_start
    }
}

/// Stan usługi Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    /// Zatrzymana.
    Stopped,
    /// Uruchamia się.
    StartPending,
    /// Zatrzymuje się.
    StopPending,
    /// Działa.
    Running,
    /// Wznawia się.
    ContinuePending,
    /// Wstrzymuje się.
    PausePending,
    /// Wstrzymana.
    Paused,
    /// Nieznany.
    Unknown,
}

impl ServiceState {
    /// Stan z `dwCurrentState` (`SERVICE_STOPPED` = 1 … `SERVICE_PAUSED` = 7).
    pub fn from_win32(state: u32) -> Self {
        match state {
            1 => Self::Stopped,
            2 => Self::StartPending,
            3 => Self::StopPending,
            4 => Self::Running,
            5 => Self::ContinuePending,
            6 => Self::PausePending,
            7 => Self::Paused,
            _ => Self::Unknown,
        }
    }
}

/// Usługa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// Nazwa usługi (klucz SCM).
    pub name: String,
    /// Nazwa wyświetlana (niezaufana).
    pub display_name: String,
    /// Stan.
    pub state: ServiceState,
    /// PID procesu usługi (gdy działa).
    pub pid: Option<u32>,
}

/// Polecenie dla usługi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceCommand {
    /// Start.
    Start,
    /// Zatrzymanie.
    Stop,
    /// Zatrzymanie i start.
    Restart,
}

/// Dziennik zdarzeń dostępny dla agentek (nigdy `Security`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLogName {
    /// `Application`.
    Application,
    /// `System`.
    System,
}

impl EventLogName {
    /// Nazwa kanału.
    pub fn channel(self) -> &'static str {
        match self {
            Self::Application => "Application",
            Self::System => "System",
        }
    }
}

/// Poziom zdarzenia (wartości `Level` z dziennika).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLevel {
    /// Krytyczne (1).
    Critical,
    /// Błąd (2).
    Error,
    /// Ostrzeżenie (3).
    Warning,
    /// Informacja (4; także 0 — „LogAlways”).
    Information,
    /// Szczegółowe (5).
    Verbose,
}

impl EventLevel {
    /// Wartość `Level`.
    pub fn value(self) -> u8 {
        match self {
            Self::Critical => 1,
            Self::Error => 2,
            Self::Warning => 3,
            Self::Information => 4,
            Self::Verbose => 5,
        }
    }

    /// Poziom z wartości `Level` (0 i nieznane → informacja).
    pub fn from_value(v: u8) -> Self {
        match v {
            1 => Self::Critical,
            2 => Self::Error,
            3 => Self::Warning,
            5 => Self::Verbose,
            _ => Self::Information,
        }
    }
}

/// Zapytanie do dziennika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventQuery {
    /// Dziennik.
    pub log: EventLogName,
    /// Najmniej ważny poziom (np. `Warning` = krytyczne, błędy i ostrzeżenia).
    pub min_level: Option<EventLevel>,
    /// Dostawca (sprawdzany `check_provider`).
    pub provider: Option<String>,
    /// Tylko zdarzenia z ostatnich `since_ms` milisekund.
    pub since_ms: Option<u64>,
    /// Najwięcej zdarzeń (≤ [`MAX_EVENTS`]); najnowsze pierwsze.
    pub max: u32,
}

/// Zdarzenie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    /// Czas (ms od epoki Unix).
    pub time_ms: u64,
    /// Poziom.
    pub level: EventLevel,
    /// Dostawca.
    pub provider: String,
    /// Identyfikator zdarzenia.
    pub event_id: u32,
    /// Komunikat (niezaufany, obcięty do [`MAX_EVENT_MESSAGE_CHARS`]).
    pub message: String,
}

/// Zakres zmiennych środowiskowych.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvScope {
    /// Środowisko procesu Alfy.
    Process,
    /// Zmienne użytkownika (`HKCU\Environment`).
    User,
    /// Zmienne systemowe (odczyt).
    Machine,
}

/// Zmienna środowiskowa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVar {
    /// Nazwa.
    pub name: String,
    /// Wartość (`None` = ukryta — nazwa sekretu).
    pub value: Option<String>,
}

/// Port systemu (Windows: `platform-windows-sys-impl::WinSys`; atrapa: `FakeSys`).
pub trait SysPort: Send + Sync {
    /// Strażnik celów (procesy Alfy, Brokera, watchdoga).
    fn guard(&self) -> &TargetGuard;

    /// Procesy.
    fn processes(&self) -> Result<Vec<ProcessEntry>, SysError>;

    /// Szczegóły procesu.
    fn process(&self, pid: u32) -> Result<ProcessDetails, SysError>;

    /// Kończy proces o tej tożsamości (chroniony → `Protected`, zmieniony → `Changed`).
    fn terminate(&self, id: &ProcessIdentity) -> Result<(), SysError>;

    /// Usługi.
    fn services(&self) -> Result<Vec<ServiceEntry>, SysError>;

    /// Polecenie dla usługi; czeka na stan końcowy najwyżej `timeout_ms`.
    fn control_service(
        &self,
        name: &str,
        command: ServiceCommand,
        timeout_ms: u64,
    ) -> Result<ServiceEntry, SysError>;

    /// Zdarzenia (najnowsze pierwsze).
    fn events(&self, query: &EventQuery) -> Result<Vec<EventRecord>, SysError>;

    /// Zmienne (wartości sekretów ukryte w porcie).
    fn env(&self, scope: EnvScope) -> Result<Vec<EnvVar>, SysError>;

    /// Surowa wartość zmiennej użytkownika (do cofania — nigdy do wyniku).
    fn user_env_value(&self, name: &str) -> Result<Option<String>, SysError>;

    /// Zapis (`Some`) albo usunięcie (`None`) zmiennej użytkownika; zwraca poprzednią wartość.
    fn set_user_env(&self, name: &str, value: Option<&str>) -> Result<Option<String>, SysError>;
}

/// Czy proces z listy jest chroniony (strażnik z łańcuchem przodków z tej listy, proces
/// krytyczny, PID systemowy).
pub fn is_protected_entry(guard: &TargetGuard, entry: &ProcessEntry, all: &[ProcessEntry]) -> bool {
    let parent_of = |pid: u32| all.iter().find(|p| p.pid == pid).map(|p| p.parent_pid);
    protected_process(guard, entry.pid, &entry.image, parent_of)
}
