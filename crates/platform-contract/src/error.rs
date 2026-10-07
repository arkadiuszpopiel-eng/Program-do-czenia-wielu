//! Błędy portu systemowego.

use std::path::PathBuf;

/// Błąd operacji systemowej. Wariant `Denylisted` jest ostatnią linią obrony
/// przed odczytem poświadczeń (AGENTS.md: nigdy `~/.claude`, `~/.codex`, ciasteczka).
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum PlatformError {
    /// Ścieżka nie istnieje.
    #[error("nie znaleziono: {0}")]
    NotFound(PathBuf),
    /// Ścieżka już istnieje (operacje nie nadpisują po cichu).
    #[error("już istnieje: {0}")]
    AlreadyExists(PathBuf),
    /// Brak uprawnień.
    #[error("brak uprawnień: {0}")]
    PermissionDenied(String),
    /// Ścieżka na deny-liście poświadczeń.
    #[error("ścieżka na deny-liście poświadczeń: {0}")]
    Denylisted(PathBuf),
    /// Ścieżka niepoprawna (pusta, względna tam, gdzie wymagana bezwzględna, itp.).
    #[error("niepoprawna ścieżka: {0}")]
    InvalidPath(PathBuf),
    /// Operacja nieodwracalna, a wywołujący zażądał odwracalności.
    #[error("operacja nieodwracalna: {0}")]
    NotReversible(String),
    /// Nieznany lub zużyty token cofnięcia.
    #[error("nieznany token cofnięcia: {0}")]
    UnknownUndoToken(u64),
    /// Skrót narusza regułę AltGr / jest zarezerwowany.
    #[error("niedozwolony skrót: {0}")]
    HotkeyRejected(String),
    /// Skrót poprawny, ale zajęty w systemie przez inną aplikację (`RegisterHotKey` odmówił).
    #[error("skrót zajęty przez inną aplikację: {0}")]
    HotkeyConflict(String),
    /// Zasób nie istnieje (okno, proces, skrót).
    #[error("nieznany zasób: {0}")]
    UnknownResource(String),
    /// Nieobsługiwane na tej platformie / w tej implementacji.
    #[error("nieobsługiwane: {0}")]
    Unsupported(String),
    /// Błąd we/wy.
    #[error("błąd we/wy: {0}")]
    Io(String),
}
