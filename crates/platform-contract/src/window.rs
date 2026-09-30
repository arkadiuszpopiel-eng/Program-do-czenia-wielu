//! Port okien.

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Identyfikator okna (HWND na Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

/// Informacja o oknie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Identyfikator.
    pub id: WindowId,
    /// Tytuł.
    pub title: String,
    /// Nazwa procesu właściciela.
    pub process: String,
    /// Czy okno jest na pierwszym planie.
    pub focused: bool,
    /// Czy okno jest pełnoekranowe (gry → STT/LLM na CPU lub chmura, PLAN §3.4).
    pub fullscreen: bool,
}

/// Port okien.
pub trait WindowPort: Send + Sync {
    /// Lista widocznych okien najwyższego poziomu.
    fn list(&self) -> Vec<WindowInfo>;

    /// Przenosi okno na pierwszy plan.
    fn focus(&self, id: WindowId) -> Result<(), PlatformError>;

    /// Czy jakaś aplikacja pełnoekranowa jest aktywna.
    fn fullscreen_app_active(&self) -> bool {
        self.list().iter().any(|w| w.focused && w.fullscreen)
    }
}
