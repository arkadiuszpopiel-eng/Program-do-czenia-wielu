//! Błędy modułu `updater` (serializowalne — trafiają do UI przez IPC).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Błędy launchera i aktualizacji.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum UpdaterError {
    /// Brak jakiejkolwiek uruchamialnej wersji (aktywna i poprzednia brakujące/uszkodzone).
    #[error("brak działającej wersji Alfy: {reason}")]
    NoUsableVersion {
        /// Szczegóły.
        reason: String,
    },
    /// Wersja nie jest zainstalowana (albo jej katalog jest uszkodzony).
    #[error("wersja {version} nie jest zainstalowana albo jest uszkodzona")]
    NotInstalled {
        /// Wersja.
        version: String,
    },
    /// Brak poprzedniej wersji do rollbacku.
    #[error("brak poprzedniej wersji do przywrócenia")]
    NoPrevious,
    /// Niezgodny skrót SHA-256 paczki aktualizacji.
    #[error("skrót SHA-256 paczki niezgodny z manifestem wydania")]
    HashMismatch,
    /// Nieprawidłowy podpis minisign (zły klucz, zmieniona paczka, brak wiązania z wersją).
    #[error("nieprawidłowy podpis minisign: {reason}")]
    SignatureInvalid {
        /// Powód.
        reason: String,
    },
    /// Brak klucza publicznego w konfiguracji — aktualizacje wyłączone.
    #[error("brak klucza publicznego minisign w konfiguracji")]
    NoPublicKey,
    /// Nieprawidłowe dane (`current.json`, manifest wydań, wersja).
    #[error("nieprawidłowe dane: {reason}")]
    Invalid {
        /// Opis.
        reason: String,
    },
    /// Błąd wejścia/wyjścia.
    #[error("we/wy: {reason}")]
    Io {
        /// Opis.
        reason: String,
    },
}

impl UpdaterError {
    /// Skrót: błąd we/wy.
    pub fn io(err: impl std::fmt::Display) -> Self {
        UpdaterError::Io {
            reason: err.to_string(),
        }
    }

    /// Skrót: nieprawidłowe dane.
    pub fn invalid(reason: impl std::fmt::Display) -> Self {
        UpdaterError::Invalid {
            reason: reason.to_string(),
        }
    }
}

impl From<std::io::Error> for UpdaterError {
    fn from(err: std::io::Error) -> Self {
        UpdaterError::io(err)
    }
}
