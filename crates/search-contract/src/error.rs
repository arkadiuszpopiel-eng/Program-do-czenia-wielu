//! Błędy wyszukiwania.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Błędy modułu `search`.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum SearchError {
    /// Wywołujący nie ma dostępu do żądanych sesji (agentka poza własną sesją).
    #[error("brak dostępu: {reason}")]
    Forbidden {
        /// Opis.
        reason: String,
    },
    /// Sesja nie istnieje.
    #[error("sesja {session} nie istnieje")]
    SessionNotFound {
        /// Identyfikator sesji.
        session: String,
    },
    /// Embedder zwrócił błąd albo wektor o złym wymiarze.
    #[error("embedder: {reason}")]
    Embedder {
        /// Opis.
        reason: String,
    },
    /// Indeks w bazie zbudowano innym embedderem (wymaga reindeksacji).
    #[error("indeks zbudowany embedderem `{indexed}`, bieżący to `{current}`")]
    EmbedderMismatch {
        /// Embedder zapisany w bazie.
        indexed: String,
        /// Bieżący embedder.
        current: String,
    },
    /// Nieprawidłowe dane.
    #[error("nieprawidłowe dane: {reason}")]
    Invalid {
        /// Opis.
        reason: String,
    },
    /// Błąd magazynu.
    #[error("magazyn: {reason}")]
    Storage {
        /// Opis.
        reason: String,
    },
}

impl SearchError {
    /// Skrót: błąd magazynu.
    pub fn storage(err: impl std::fmt::Display) -> Self {
        SearchError::Storage {
            reason: err.to_string(),
        }
    }
}
