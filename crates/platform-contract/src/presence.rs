//! Obecność użytkownika poza bezczynnością: tryb pełnego ekranu / gry (PLAN §3.4 „bez wyścigu
//! o VRAM z grami”; sygnał dla `model-residency` i okien schedulera) oraz blokada stacji / sesji
//! (wyciszenie głosu, wstrzymanie computer use).
//!
//! Windows: `SHQueryUserNotificationState` (`QUNS_*`) + okno pierwszego planu pokrywające monitor;
//! `WTSQuerySessionInformationW(WTSSessionInfoEx)` + `WM_WTSSESSION_CHANGE`.

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Domyślny czas utrzymania trybu gry po zniknięciu pełnego ekranu (alt-tab nie przełącza
/// modeli tam i z powrotem).
pub const DEFAULT_GAME_EXIT_AFTER_MS: u64 = 10_000;

/// Stan powiadomień użytkownika (`QUERY_USER_NOTIFICATION_STATE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationState {
    /// `QUNS_NOT_PRESENT` (1): wygaszacz, blokada, przełączanie użytkowników.
    NotPresent,
    /// `QUNS_BUSY` (2): aplikacja pełnoekranowa albo ustawienia prezentacji.
    Busy,
    /// `QUNS_RUNNING_D3D_FULL_SCREEN` (3): gra Direct3D w wyłącznym pełnym ekranie.
    D3dFullScreen,
    /// `QUNS_PRESENTATION_MODE` (4): tryb prezentacji.
    PresentationMode,
    /// `QUNS_ACCEPTS_NOTIFICATIONS` (5): zwykła praca.
    AcceptsNotifications,
    /// `QUNS_QUIET_TIME` (6): cisza po instalacji systemu.
    QuietTime,
    /// `QUNS_APP` (7): aplikacja ze sklepu na pierwszym planie.
    App,
    /// Wartość spoza listy albo błąd zapytania.
    Unknown,
}

impl NotificationState {
    /// Z wartości `QUERY_USER_NOTIFICATION_STATE`.
    pub fn from_raw(value: i32) -> Self {
        match value {
            1 => Self::NotPresent,
            2 => Self::Busy,
            3 => Self::D3dFullScreen,
            4 => Self::PresentationMode,
            5 => Self::AcceptsNotifications,
            6 => Self::QuietTime,
            7 => Self::App,
            _ => Self::Unknown,
        }
    }
}

/// Dlaczego tryb gry jest aktywny.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameReason {
    /// Gra Direct3D w wyłącznym pełnym ekranie.
    D3dFullScreen,
    /// Tryb prezentacji.
    Presentation,
    /// Aplikacja pełnoekranowa (także okno bez ramki — gry „borderless”, wideo).
    Busy,
    /// Okno pierwszego planu pokrywa cały monitor (bez ramki; nie pulpit i nie okno Alfy).
    ForegroundFullscreen,
}

/// Próbka trybu pełnego ekranu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullscreenProbe {
    /// Stan powiadomień (`SHQueryUserNotificationState`).
    pub notification: NotificationState,
    /// Okno pierwszego planu pokrywa cały monitor (pulpit, pasek zadań i okna Alfy wykluczone).
    pub foreground_fullscreen: bool,
    /// Obraz procesu okna pierwszego planu (np. `gra.exe`) — tylko diagnostyka, nie loguj ścieżek.
    #[serde(default)]
    pub foreground_image: Option<String>,
}

impl FullscreenProbe {
    /// Nic na pełnym ekranie.
    pub fn none() -> Self {
        Self {
            notification: NotificationState::AcceptsNotifications,
            foreground_fullscreen: false,
            foreground_image: None,
        }
    }

    /// Powód trybu gry (`None` = brak). Wygaszacz/blokada (`NotPresent`) nie jest grą.
    pub fn game_reason(&self) -> Option<GameReason> {
        match self.notification {
            NotificationState::D3dFullScreen => Some(GameReason::D3dFullScreen),
            NotificationState::PresentationMode => Some(GameReason::Presentation),
            NotificationState::Busy => Some(GameReason::Busy),
            NotificationState::NotPresent => None,
            _ if self.foreground_fullscreen => Some(GameReason::ForegroundFullscreen),
            _ => None,
        }
    }
}

/// Port trybu pełnego ekranu.
pub trait FullscreenPort: Send + Sync {
    /// Bieżąca próbka.
    fn probe(&self) -> Result<FullscreenProbe, PlatformError>;
}

/// Tryb gry z histerezą: wejście natychmiast (zwolnij VRAM jak najszybciej), wyjście po
/// `exit_after_ms` bez pełnego ekranu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameModeTracker {
    exit_after_ms: u64,
    active: Option<GameReason>,
    last_seen_ms: u64,
}

impl GameModeTracker {
    /// Nowy tracker.
    pub fn new(exit_after_ms: u64) -> Self {
        Self {
            exit_after_ms,
            active: None,
            last_seen_ms: 0,
        }
    }

    /// Aktywny powód (po histerezie).
    pub fn active(&self) -> Option<GameReason> {
        self.active
    }

    /// Odczyt nieudany: aktywny tryb gry trwa (brak danych nie rozpoczyna odliczania wyjścia).
    pub fn hold(&mut self, now_ms: u64) {
        if self.active.is_some() {
            self.last_seen_ms = now_ms;
        }
    }

    /// Próbka; `Some(nowy stan)` przy zmianie aktywności albo powodu.
    pub fn observe(
        &mut self,
        now_ms: u64,
        reason: Option<GameReason>,
    ) -> Option<Option<GameReason>> {
        match reason {
            Some(r) => {
                self.last_seen_ms = now_ms;
                (self.active != Some(r)).then(|| {
                    self.active = Some(r);
                    self.active
                })
            }
            None if self.active.is_some()
                && now_ms.saturating_sub(self.last_seen_ms) >= self.exit_after_ms =>
            {
                self.active = None;
                Some(None)
            }
            None => None,
        }
    }
}

/// Stan sesji użytkownika.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Sesja aktywna, odblokowana.
    Active,
    /// Stacja zablokowana (`Win+L`, wygaszacz z hasłem).
    Locked,
    /// Sesja rozłączona (przełączenie użytkownika, RDP rozłączone).
    Disconnected,
    /// Nieznany (błąd zapytania) — monitor zachowuje ostatni znany stan.
    Unknown,
}

impl SessionState {
    /// Z `WTSINFOEX_LEVEL1_W`: `SessionState` (0 aktywna, 4 rozłączona) i `SessionFlags`
    /// (0 = `WTS_SESSIONSTATE_LOCK`, 1 = `WTS_SESSIONSTATE_UNLOCK`, −1 nieznane).
    pub fn from_wts(connect_state: i32, session_flags: i32) -> Self {
        if connect_state == 4 {
            return Self::Disconnected;
        }
        match session_flags {
            0 => Self::Locked,
            1 => Self::Active,
            _ => Self::Unknown,
        }
    }

    /// Użytkownik nieobecny przy ekranie (zablokowane albo rozłączone): głos wyciszony,
    /// computer use wstrzymany.
    pub fn is_away(self) -> bool {
        matches!(self, Self::Locked | Self::Disconnected)
    }
}

/// Port stanu sesji.
pub trait SessionPort: Send + Sync {
    /// Bieżący stan sesji procesu.
    fn session(&self) -> Result<SessionState, PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_states_map_to_game_reasons() {
        let probe = |n: i32, fg: bool| FullscreenProbe {
            notification: NotificationState::from_raw(n),
            foreground_fullscreen: fg,
            foreground_image: None,
        };
        assert_eq!(
            probe(3, false).game_reason(),
            Some(GameReason::D3dFullScreen)
        );
        assert_eq!(
            probe(4, false).game_reason(),
            Some(GameReason::Presentation)
        );
        assert_eq!(probe(2, false).game_reason(), Some(GameReason::Busy));
        assert_eq!(probe(1, true).game_reason(), None);
        assert_eq!(
            probe(5, true).game_reason(),
            Some(GameReason::ForegroundFullscreen)
        );
        assert_eq!(probe(5, false).game_reason(), None);
        assert_eq!(probe(99, false).notification, NotificationState::Unknown);
        assert_eq!(FullscreenProbe::none().game_reason(), None);
    }

    #[test]
    fn game_mode_enters_at_once_and_leaves_after_delay() {
        let mut g = GameModeTracker::new(10_000);
        assert_eq!(g.observe(0, None), None);
        assert_eq!(
            g.observe(1_000, Some(GameReason::Busy)),
            Some(Some(GameReason::Busy))
        );
        assert_eq!(g.observe(2_000, Some(GameReason::Busy)), None);
        assert_eq!(g.observe(5_000, None), None);
        assert_eq!(
            g.observe(6_000, Some(GameReason::D3dFullScreen)),
            Some(Some(GameReason::D3dFullScreen))
        );
        assert_eq!(g.observe(15_999, None), None);
        assert_eq!(g.observe(16_000, None), Some(None));
        assert_eq!(g.active(), None);
    }

    #[test]
    fn session_from_wts() {
        assert_eq!(SessionState::from_wts(0, 1), SessionState::Active);
        assert_eq!(SessionState::from_wts(0, 0), SessionState::Locked);
        assert_eq!(SessionState::from_wts(4, 1), SessionState::Disconnected);
        assert_eq!(SessionState::from_wts(0, -1), SessionState::Unknown);
        assert!(SessionState::Locked.is_away() && SessionState::Disconnected.is_away());
        assert!(!SessionState::Active.is_away() && !SessionState::Unknown.is_away());
    }
}
