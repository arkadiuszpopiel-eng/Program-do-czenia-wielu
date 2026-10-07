//! Błędy pamięci.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::MemoryId;

/// Błędy pamięci.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum MemoryError {
    /// Funkcja poza v0 (zakres inny niż sesja, warstwa robocza/proceduralna, awans).
    #[error("nieobsługiwane w v0: {what}")]
    Unsupported {
        /// Czego dotyczy.
        what: String,
    },
    /// Wpis nie istnieje.
    #[error("wpis {id} nie istnieje")]
    NotFound {
        /// Identyfikator.
        id: MemoryId,
    },
    /// Wpis z treści niezaufanej nie może awansować do innego zakresu.
    #[error("wpis z treści niezaufanej nie może awansować")]
    UntrustedCannotPromote,
    /// Automatyczne zapamiętywanie z treści niezaufanej jest wyłączone.
    #[error("automatyczne zapamiętywanie z treści niezaufanej jest wyłączone")]
    UntrustedAutoRemember,
    /// Nieprawidłowe dane.
    #[error("nieprawidłowe dane: {reason}")]
    Invalid {
        /// Opis.
        reason: String,
    },
    /// Wywołujący nie ma uprawnień do zakresu lub operacji (F7).
    #[error("brak dostępu: {reason}")]
    Forbidden {
        /// Opis.
        reason: String,
    },
    /// Treść z sesji prywatnej nie może zasilić zakresu szerszego (F7).
    #[error("treść sesji prywatnej {session} nie może trafić do zakresu szerszego")]
    PrivateSource {
        /// Sesja źródłowa.
        session: String,
    },
    /// Stan wpisu nie pozwala na operację (np. zastąpiony, oczekujący, nieodwracalny; F7).
    #[error("konflikt stanu: {reason}")]
    Conflict {
        /// Opis.
        reason: String,
    },
    /// Błąd magazynu lub indeksu.
    #[error("magazyn: {reason}")]
    Storage {
        /// Opis.
        reason: String,
    },
}

impl MemoryError {
    /// Skrót: błąd magazynu.
    pub fn storage(e: impl fmt::Display) -> Self {
        MemoryError::Storage {
            reason: e.to_string(),
        }
    }

    /// Skrót: nieprawidłowe dane.
    pub fn invalid(reason: impl Into<String>) -> Self {
        MemoryError::Invalid {
            reason: reason.into(),
        }
    }

    /// Skrót: brak dostępu.
    pub fn forbidden(reason: impl Into<String>) -> Self {
        MemoryError::Forbidden {
            reason: reason.into(),
        }
    }

    /// Skrót: konflikt stanu.
    pub fn conflict(reason: impl Into<String>) -> Self {
        MemoryError::Conflict {
            reason: reason.into(),
        }
    }
}
