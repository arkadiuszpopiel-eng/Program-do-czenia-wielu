//! `WindowPort`: lista widocznych okien najwyższego poziomu (tytuł, proces, PID, prostokąt,
//! monitor, DPI), fokus, minimalizacja i przywracanie — z zakazem operacji na oknach procesów
//! Alfy i Brokera (AGENTS.md: zakaz `gui.control` wobec procesów Alfy/Brokera).

#[cfg(windows)]
mod win;

use platform_contract::{PlatformError, WindowId, WindowInfo, WindowPort};
use serde::{Deserialize, Serialize};

/// Domyślne nazwy procesów chronionych (porównanie bez wielkości liter).
pub const DEFAULT_PROTECTED_PROCESSES: [&str; 6] = [
    "alfa.exe",
    "alfa-desktop.exe",
    "alfa-broker.exe",
    "alfa-broker-ui.exe",
    "alfa-watchdog.exe",
    "alfa-updater.exe",
];

/// Prostokąt w pikselach ekranu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    /// Lewa krawędź.
    pub left: i32,
    /// Górna krawędź.
    pub top: i32,
    /// Prawa krawędź.
    pub right: i32,
    /// Dolna krawędź.
    pub bottom: i32,
}

impl Rect {
    /// Czy `self` w całości pokrywa `other`.
    pub fn covers(&self, other: &Rect) -> bool {
        self.left <= other.left
            && self.top <= other.top
            && self.right >= other.right
            && self.bottom >= other.bottom
    }
}

/// Pełny opis okna (ponad `WindowInfo` z kontraktu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowDetails {
    /// Część kontraktowa.
    pub info: WindowInfo,
    /// PID procesu właściciela.
    pub pid: u32,
    /// Prostokąt okna.
    pub rect: Rect,
    /// Identyfikator monitora (HMONITOR).
    pub monitor: u64,
    /// Prostokąt monitora.
    pub monitor_rect: Rect,
    /// Czy to monitor główny.
    pub primary_monitor: bool,
    /// DPI okna (96 = 100%).
    pub dpi: u32,
    /// Czy okno jest zminimalizowane.
    pub minimized: bool,
    /// Czy okno należy do chronionego procesu (Alfa/Broker).
    pub protected: bool,
}

/// Okna chronione przed sterowaniem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowGuard {
    /// Nazwy plików wykonywalnych (bez wielkości liter).
    pub protected_processes: Vec<String>,
    /// Dodatkowe PID-y (bieżący proces jest chroniony zawsze).
    pub protected_pids: Vec<u32>,
}

impl Default for WindowGuard {
    fn default() -> Self {
        Self {
            protected_processes: DEFAULT_PROTECTED_PROCESSES.map(String::from).to_vec(),
            protected_pids: Vec::new(),
        }
    }
}

impl WindowGuard {
    /// Czy proces (PID, nazwa pliku) jest chroniony.
    pub fn is_protected(&self, pid: u32, process_name: &str) -> bool {
        pid == std::process::id()
            || self.protected_pids.contains(&pid)
            || self
                .protected_processes
                .iter()
                .any(|p| p.eq_ignore_ascii_case(process_name))
    }

    fn ensure_allowed(&self, pid: u32, name: &str, id: WindowId) -> Result<(), PlatformError> {
        if self.is_protected(pid, name) {
            return Err(PlatformError::PermissionDenied(format!(
                "okno {} należy do chronionego procesu {name} (Alfa/Broker)",
                id.0
            )));
        }
        Ok(())
    }
}

/// Akcja na oknie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowAction {
    Focus,
    Minimize,
    Restore,
}

/// Okna Windows.
#[derive(Debug, Default)]
pub struct WinWindows {
    guard: WindowGuard,
}

impl WinWindows {
    /// Nowy port okien z listą procesów chronionych.
    pub fn new(guard: WindowGuard) -> Self {
        Self { guard }
    }

    /// Szczegółowa lista okien (pusta poza Windows).
    pub fn list_detailed(&self) -> Vec<WindowDetails> {
        #[cfg(windows)]
        {
            win::enumerate(&self.guard)
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }

    /// Minimalizuje okno.
    pub fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
        self.act(id, WindowAction::Minimize)
    }

    /// Przywraca okno (z minimalizacji lub maksymalizacji).
    pub fn restore(&self, id: WindowId) -> Result<(), PlatformError> {
        self.act(id, WindowAction::Restore)
    }

    fn act(&self, id: WindowId, action: WindowAction) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            let (pid, name) = win::owner(id)?;
            self.guard.ensure_allowed(pid, &name, id)?;
            win::apply(id, action)
        }
        #[cfg(not(windows))]
        {
            let _ = (action, &self.guard);
            Err(PlatformError::Unsupported(format!(
                "okno {}: sterowanie oknami tylko na Windows",
                id.0
            )))
        }
    }
}

impl WindowPort for WinWindows {
    fn list(&self) -> Vec<WindowInfo> {
        self.list_detailed().into_iter().map(|d| d.info).collect()
    }

    fn focus(&self, id: WindowId) -> Result<(), PlatformError> {
        self.act(id, WindowAction::Focus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_protects_own_process_and_alfa_binaries() {
        let guard = WindowGuard {
            protected_pids: vec![4242],
            ..WindowGuard::default()
        };
        assert!(guard.is_protected(std::process::id(), "anything.exe"));
        assert!(guard.is_protected(4242, "notepad.exe"));
        assert!(guard.is_protected(7, "ALFA-BROKER.EXE"));
        assert!(!guard.is_protected(7, "notepad.exe"));
        let err = guard
            .ensure_allowed(7, "alfa.exe", WindowId(1))
            .unwrap_err();
        assert!(matches!(err, PlatformError::PermissionDenied(m) if m.contains("alfa.exe")));
    }

    #[test]
    fn rect_cover_and_non_windows_behaviour() {
        let monitor = Rect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let full = Rect {
            left: -8,
            top: -8,
            right: 1928,
            bottom: 1088,
        };
        assert!(full.covers(&monitor));
        assert!(!monitor.covers(&full));
        let w = WinWindows::new(WindowGuard::default());
        if !cfg!(windows) {
            assert!(w.list().is_empty());
            assert!(!w.fullscreen_app_active());
            assert!(w.focus(WindowId(1)).is_err());
            assert!(w.minimize(WindowId(1)).is_err());
            assert!(w.restore(WindowId(1)).is_err());
        }
    }
}
