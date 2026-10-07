//! Nadzór: heartbeat, polityka restartów, safe-mode, rollback, porty do `core-config`,
//! `updater` i uruchamiania procesów (PLAN §8.6, §12.2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::kill::{JobRegistry, KillSwitch, ProcessRole};

/// Zdarzenie: brak heartbeatu w terminie.
pub const EVENT_HEARTBEAT_MISSED: &str = "watchdog.heartbeat.missed";
/// Zdarzenie: restart procesu.
pub const EVENT_RESTART: &str = "watchdog.restart";
/// Zdarzenie: pętla awarii.
pub const EVENT_CRASH_LOOP: &str = "watchdog.crash_loop";
/// Zdarzenie (Audyt): wejście w safe-mode.
pub const EVENT_SAFE_MODE_ENTERED: &str = "watchdog.safe_mode.entered";
/// Zdarzenie (Audyt): wyjście z safe-mode.
pub const EVENT_SAFE_MODE_LEFT: &str = "watchdog.safe_mode.left";
/// Zdarzenie (Audyt): rollback konfiguracji lub wersji.
pub const EVENT_ROLLBACK: &str = "watchdog.rollback";

/// Stan zdrowia zgłaszany w heartbeacie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "health", content = "detail", rename_all = "snake_case")]
pub enum Health {
    /// Działa.
    Ok,
    /// Działa z ograniczeniami.
    Degraded(String),
    /// Nie działa (traktowane jak awaria).
    Failing(String),
}

/// Heartbeat procesu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Heartbeat {
    /// Nadawca.
    pub from: ProcessRole,
    /// Stan.
    pub health: Health,
}

/// Polityka watchdoga (`[watchdog]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WatchPolicy {
    /// Brak heartbeatu dłużej niż tyle = awaria (domyślnie 5 s).
    pub heartbeat_timeout_ms: u64,
    /// Ile restartów w oknie, zanim uznamy pętlę awarii (domyślnie 3).
    pub max_restarts: u8,
    /// Okno liczenia restartów (domyślnie 10 min).
    pub window_ms: u64,
    /// Minimalny odstęp między rollbackami (domyślnie 30 min) — nigdy w pętli.
    pub cooldown_ms: u64,
    /// Czy po pętli awarii wejść w safe-mode.
    pub safe_mode_after_crash_loop: bool,
    /// Czy po pętli awarii automatycznie cofnąć konfigurację/wersję do ostatniej dobrej.
    pub auto_rollback: bool,
}

impl Default for WatchPolicy {
    fn default() -> Self {
        Self {
            heartbeat_timeout_ms: 5_000,
            max_restarts: 3,
            window_ms: 10 * 60 * 1000,
            cooldown_ms: 30 * 60 * 1000,
            safe_mode_after_crash_loop: true,
            auto_rollback: true,
        }
    }
}

/// Akcja podjęta przez watchdoga (log + wykonanie przez porty).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WatchAction {
    /// Restart procesu (moduł albo jądro).
    Restart {
        /// Kto.
        role: ProcessRole,
        /// Numer restartu w bieżącym oknie.
        attempt: u8,
    },
    /// Wejście w safe-mode (tylko procesy krytyczne).
    EnterSafeMode {
        /// Powód.
        reason: String,
    },
    /// Zatrzymanie procesu niekrytycznego w safe-mode.
    StopForSafeMode {
        /// Kto.
        role: ProcessRole,
    },
    /// Wyjście z safe-mode (ręczne).
    LeaveSafeMode,
    /// Rollback konfiguracji do ostatniej dobrej rewizji.
    RollbackConfig {
        /// Rewizja docelowa.
        revision: String,
    },
    /// Sygnał do `updater`: rollback wersji do ostatniej dobrej.
    RollbackVersion {
        /// Wersja docelowa.
        to: String,
    },
    /// Rollback pominięty (cooldown albo brak „ostatniej dobrej”).
    RollbackSkipped {
        /// Powód.
        reason: String,
    },
}

/// Stan safe-mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SafeModeState {
    /// Od kiedy (ms).
    pub since_ms: u64,
    /// Powód.
    pub reason: String,
}

/// Port do historii konfiguracji (`core-config`).
pub trait ConfigHistory: Send + Sync {
    /// Bieżąca rewizja konfiguracji.
    fn current_revision(&self) -> Option<String>;
    /// Przywraca rewizję.
    fn rollback_to(&self, revision: &str) -> Result<(), String>;
}

/// Port do aktualizatora (`updater`): sygnał rollbacku wersji.
pub trait UpdaterSignal: Send + Sync {
    /// Bieżąca wersja programu.
    fn current_version(&self) -> String;
    /// Zleca rollback do wersji.
    fn request_rollback(&self, to: &str) -> Result<(), String>;
}

/// Port uruchamiania/zatrzymywania procesów (launcher).
pub trait Supervisor: Send + Sync {
    /// Uruchamia ponownie proces.
    fn restart(&self, role: &ProcessRole) -> Result<(), String>;
    /// Zatrzymuje proces (safe-mode).
    fn stop(&self, role: &ProcessRole) -> Result<(), String>;
}

/// Ręczne potwierdzenie wyjścia z safe-mode (akcja właściciela w zasobniku/UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ManualConfirmation {
    /// Skąd przyszło potwierdzenie (np. `tray`).
    pub surface: String,
}

/// Błędy watchdoga.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WatchdogError {
    /// Nadawca nie jest nadzorowany.
    #[error("proces {0} nie jest nadzorowany")]
    NotWatched(ProcessRole),
    /// Nie ma safe-mode, z którego można wyjść.
    #[error("watchdog nie jest w safe-mode")]
    NotInSafeMode,
    /// Proces niekrytyczny w safe-mode.
    #[error("safe-mode: proces {0} nie może działać")]
    BlockedBySafeMode(ProcessRole),
}

/// Watchdog: nadzór + rejestr Job Objects + kill-switch.
pub trait Watchdog: KillSwitch + JobRegistry {
    /// Dodaje proces do nadzoru (`critical` = działa także w safe-mode).
    fn watch(&self, role: ProcessRole, critical: bool);
    /// Heartbeat; `Health::Failing` jest traktowane jak awaria.
    fn heartbeat(&self, hb: Heartbeat) -> Result<Vec<WatchAction>, WatchdogError>;
    /// Zgłoszenie awarii (proces zakończył się nieoczekiwanie).
    fn report_crash(&self, role: &ProcessRole, detail: &str) -> Vec<WatchAction>;
    /// Sprawdza terminy heartbeatów (wywoływane co sekundę).
    fn tick(&self) -> Vec<WatchAction>;
    /// Oznacza bieżącą konfigurację i wersję jako „ostatnią dobrą” (health po starcie).
    fn mark_last_good(&self);
    /// Stan safe-mode (`None` = normalna praca).
    fn safe_mode(&self) -> Option<SafeModeState>;
    /// Czy proces może zostać uruchomiony (safe-mode blokuje niekrytyczne).
    fn may_start(&self, role: &ProcessRole) -> bool;
    /// Ręczne wyjście z safe-mode.
    fn leave_safe_mode(&self, confirmation: ManualConfirmation) -> Result<(), WatchdogError>;
    /// Dziennik podjętych akcji (od startu).
    fn action_log(&self) -> Vec<WatchAction>;
}
