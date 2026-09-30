//! Port zasobnika systemowego i powiadomień.

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Stan ikony zasobnika (odzwierciedla stan systemowy, PLAN §14.4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayState {
    /// Bezczynność.
    #[default]
    Idle,
    /// Słucha (mikrofon aktywny).
    Listening,
    /// Pracuje (agentka wykonuje zadanie).
    Working,
    /// Czeka na zatwierdzenie.
    AwaitingApproval,
    /// Błąd.
    Error,
}

/// Pozycja menu zasobnika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayMenuItem {
    /// Identyfikator akcji.
    pub id: String,
    /// Etykieta (i18n po stronie UI).
    pub label: String,
    /// Czy aktywna.
    pub enabled: bool,
}

/// Powiadomienie (toast).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    /// Tytuł.
    pub title: String,
    /// Treść.
    pub body: String,
}

/// Port zasobnika.
pub trait TrayPort: Send + Sync {
    /// Ustawia stan ikony.
    fn set_state(&self, state: TrayState) -> Result<(), PlatformError>;

    /// Bieżący stan ikony.
    fn state(&self) -> TrayState;

    /// Ustawia menu.
    fn set_menu(&self, items: Vec<TrayMenuItem>) -> Result<(), PlatformError>;

    /// Pokazuje powiadomienie.
    fn notify(&self, notification: Notification) -> Result<(), PlatformError>;
}
