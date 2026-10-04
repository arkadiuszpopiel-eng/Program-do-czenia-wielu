//! Błędy embeddera.

use search_contract::SearchError;

/// Błędy `lib-embed`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbedError {
    /// Niepoprawny manifest modelu.
    #[error("manifest: {0}")]
    Manifest(String),
    /// Błąd odczytu/zapisu pliku.
    #[error("plik {path}: {reason}")]
    Io {
        /// Ścieżka.
        path: String,
        /// Opis.
        reason: String,
    },
    /// Hash pliku niezgodny z manifestem (plik nie jest ładowany).
    #[error("SHA-256 pliku {path}: {actual} ≠ {expected}")]
    Hash {
        /// Ścieżka.
        path: String,
        /// Oczekiwany (manifest).
        expected: String,
        /// Rzeczywisty.
        actual: String,
    },
    /// Tokenizer (format `tokenizer.json`).
    #[error("tokenizer: {0}")]
    Tokenizer(String),
    /// Model ONNX (ładowanie lub wykonanie).
    #[error("model: {0}")]
    Model(String),
    /// Zarządca rezydencji odmówił miejsca w RAM.
    #[error("rezydencja: {0}")]
    Residency(String),
    /// Wątek embeddera zatrzymany.
    #[error("embedder zatrzymany")]
    Stopped,
    /// Pobieranie pliku modelu.
    #[error("pobieranie: {0}")]
    Fetch(String),
    /// Operacja anulowana.
    #[error("anulowano")]
    Cancelled,
}

impl EmbedError {
    /// Skrót: błąd pliku.
    pub fn io(path: &std::path::Path, err: impl std::fmt::Display) -> Self {
        EmbedError::Io {
            path: path.display().to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<EmbedError> for SearchError {
    fn from(e: EmbedError) -> Self {
        SearchError::Embedder {
            reason: e.to_string(),
        }
    }
}
