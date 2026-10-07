//! Błędy modułu MCP.

use crate::trust::ConsentReason;

/// Błąd klienta, serwera albo hosta MCP. Komunikaty nie zawierają tokenów ani argumentów narzędzi.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum McpError {
    /// Nie udało się uruchomić serwera.
    #[error("nie udało się uruchomić serwera MCP: {0}")]
    Spawn(String),
    /// Błąd transportu (we/wy).
    #[error("błąd transportu MCP: {0}")]
    Transport(String),
    /// Połączenie zamknięte.
    #[error("połączenie MCP zamknięte")]
    Closed,
    /// Przekroczony czas żądania.
    #[error("przekroczony czas żądania `{method}`")]
    Timeout {
        /// Metoda.
        method: String,
    },
    /// Naruszenie protokołu przez drugą stronę.
    #[error("naruszenie protokołu MCP: {0}")]
    Protocol(String),
    /// Błąd JSON-RPC zwrócony przez drugą stronę.
    #[error("błąd JSON-RPC {code}: {message}")]
    Rpc {
        /// Kod.
        code: i64,
        /// Komunikat.
        message: String,
    },
    /// Serwer wybrał nieobsługiwaną wersję protokołu.
    #[error("nieobsługiwana wersja protokołu MCP `{0}`")]
    UnsupportedProtocolVersion(String),
    /// Nieznane narzędzie.
    #[error("nieznane narzędzie `{0}`")]
    UnknownTool(String),
    /// Narzędzie zablokowane do (ponownej) zgody użytkownika.
    #[error("narzędzie `{tool}` wymaga zgody użytkownika ({reason:?})")]
    NeedsConsent {
        /// Narzędzie.
        tool: String,
        /// Powód.
        reason: ConsentReason,
    },
    /// Odcisk podany przy zgodzie nie odpowiada bieżącej definicji narzędzia.
    #[error("odcisk narzędzia `{0}` nie odpowiada bieżącej definicji — odśwież listę")]
    FingerprintMismatch(String),
    /// Zgody może udzielić tylko użytkownik.
    #[error("zgody na narzędzie MCP może udzielić tylko użytkownik")]
    ConsentNotPermitted,
    /// Token sesyjny nieznany, wygasły albo unieważniony.
    #[error("token sesyjny odrzucony: {0}")]
    Unauthorized(String),
    /// Konfiguracja niepoprawna (np. identyfikator serwera, punkt końcowy).
    #[error("niepoprawna konfiguracja MCP: {0}")]
    InvalidConfig(String),
}
