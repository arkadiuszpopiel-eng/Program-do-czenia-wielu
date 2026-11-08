//! Port okien v2 (F6, PLAN §7.2 „okna i pulpity”): lista okien najwyższego poziomu z PID,
//! obrazem procesu, prostokątem, stanem i monitorem; okno na pierwszym planie; monitory z DPI;
//! fokus, przesuwanie/rozmiar, minimalizacja/maksymalizacja/przywracanie — każda zmiana przez
//! [`TargetGuard`] (zakaz wobec okien Alfy/Brokera), sprawdzany tuż przed wywołaniem systemu.

use serde::{Deserialize, Serialize};

use crate::gui::{GuiError, ScreenRect, TargetGuard};
use crate::window::WindowId;

/// Stan okna.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowState {
    /// Zwykłe.
    #[default]
    Normal,
    /// Zminimalizowane.
    Minimized,
    /// Zmaksymalizowane.
    Maximized,
}

/// Okno najwyższego poziomu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopWindow {
    /// Identyfikator (HWND).
    pub id: WindowId,
    /// Tytuł (treść niezaufana — pochodzi z aplikacji).
    pub title: String,
    /// Klasa okna.
    pub class_name: String,
    /// PID procesu właściciela.
    pub pid: u32,
    /// Obraz procesu (pełna ścieżka, jeśli znana; pusty = nieznany → chroniony).
    pub image: String,
    /// Prostokąt (ramka DWM, piksele fizyczne).
    pub rect: ScreenRect,
    /// Indeks monitora (kolejność z [`DesktopPort::monitors`]).
    pub monitor: u32,
    /// DPI okna (96 = 100%).
    pub dpi: u32,
    /// Stan.
    pub state: WindowState,
    /// Czy na pierwszym planie.
    pub focused: bool,
    /// Pozycja w kolejności Z (0 = najwyżej).
    pub z_order: u32,
    /// Czy proces jest podniesiony (UIPI blokuje wejście bez helpera `uiAccess`).
    pub elevated: bool,
    /// Czy chroniony przez [`TargetGuard`] (Alfa, Broker, nieznany proces).
    pub protected: bool,
}

/// Monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Indeks (0 = pierwszy z wyliczenia).
    pub index: u32,
    /// Prostokąt monitora.
    pub rect: ScreenRect,
    /// Obszar roboczy (bez paska zadań).
    pub work_area: ScreenRect,
    /// Efektywne DPI.
    pub dpi: u32,
    /// Czy główny.
    pub primary: bool,
}

/// Najmniejszy dopuszczalny rozmiar okna przy `set_bounds` (px).
pub const MIN_WINDOW_SIZE: i32 = 64;

/// Sprawdza docelowy prostokąt okna: rozmiar ≥ [`MIN_WINDOW_SIZE`] i część wspólna z którymś
/// monitorem ≥ 64×64 (okno nie „ucieka” poza ekran).
pub fn validate_bounds(rect: &ScreenRect, monitors: &[MonitorInfo]) -> Result<(), GuiError> {
    if rect.width() < MIN_WINDOW_SIZE || rect.height() < MIN_WINDOW_SIZE {
        return Err(GuiError::Policy(format!(
            "okno musi mieć co najmniej {MIN_WINDOW_SIZE}×{MIN_WINDOW_SIZE} px"
        )));
    }
    let visible = monitors.iter().any(|m| {
        m.rect
            .intersect(rect)
            .is_some_and(|i| i.width() >= MIN_WINDOW_SIZE && i.height() >= MIN_WINDOW_SIZE)
    });
    if !visible {
        return Err(GuiError::Policy(
            "okno znalazłoby się poza ekranem — wybierz położenie na monitorze".into(),
        ));
    }
    Ok(())
}

/// Port okien v2.
pub trait DesktopPort: Send + Sync {
    /// Strażnik celów używany przez implementację.
    fn guard(&self) -> &TargetGuard;

    /// Widoczne okna najwyższego poziomu w kolejności Z (od góry), także chronione
    /// (`protected = true`; potrzebne do maskowania zrzutów).
    fn windows(&self) -> Result<Vec<DesktopWindow>, GuiError>;

    /// Jedno okno.
    fn window(&self, id: WindowId) -> Result<DesktopWindow, GuiError> {
        self.windows()?
            .into_iter()
            .find(|w| w.id == id)
            .ok_or_else(|| GuiError::ElementNotFound(format!("okno {}", id.0)))
    }

    /// Okno na pierwszym planie (brak = pulpit / ekran blokady).
    fn foreground(&self) -> Result<Option<DesktopWindow>, GuiError>;

    /// Okno najwyższego poziomu pod punktem ekranu.
    fn window_at(&self, x: i32, y: i32) -> Result<Option<WindowId>, GuiError>;

    /// Monitory.
    fn monitors(&self) -> Result<Vec<MonitorInfo>, GuiError>;

    /// Przenosi okno na pierwszy plan (przywraca z minimalizacji).
    fn focus(&self, id: WindowId) -> Result<(), GuiError>;

    /// Ustawia położenie i rozmiar ([`validate_bounds`]); okno zmaksymalizowane najpierw przywraca.
    fn set_bounds(&self, id: WindowId, rect: ScreenRect) -> Result<(), GuiError>;

    /// Minimalizuje, maksymalizuje albo przywraca okno.
    fn set_state(&self, id: WindowId, state: WindowState) -> Result<(), GuiError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_must_stay_on_screen() {
        let m = MonitorInfo {
            index: 0,
            rect: ScreenRect::from_xywh(0, 0, 1920, 1080),
            work_area: ScreenRect::from_xywh(0, 0, 1920, 1040),
            dpi: 96,
            primary: true,
        };
        assert!(validate_bounds(&ScreenRect::from_xywh(100, 100, 800, 600), &[m]).is_ok());
        assert!(validate_bounds(&ScreenRect::from_xywh(100, 100, 10, 600), &[m]).is_err());
        assert!(validate_bounds(&ScreenRect::from_xywh(1900, 100, 800, 600), &[m]).is_err());
        assert!(validate_bounds(&ScreenRect::from_xywh(5000, 100, 800, 600), &[m]).is_err());
        assert!(validate_bounds(&ScreenRect::from_xywh(0, 0, 800, 600), &[]).is_err());
        assert_eq!(WindowState::default(), WindowState::Normal);
    }
}
