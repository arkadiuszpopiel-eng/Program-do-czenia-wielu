//! Port schowka.

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Zawartość schowka (na start: tekst, lista plików, obraz jako bajty PNG).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ClipboardContent {
    /// Schowek pusty.
    Empty,
    /// Tekst UTF-8.
    Text(String),
    /// Lista ścieżek plików.
    Files(Vec<std::path::PathBuf>),
    /// Obraz PNG.
    ImagePng(Vec<u8>),
}

/// Port schowka. Zapis jest odwracalny przez `restore_previous` (historia w `tools-clipboard`).
pub trait ClipboardPort: Send + Sync {
    /// Bieżąca zawartość.
    fn get(&self) -> Result<ClipboardContent, PlatformError>;

    /// Ustawia zawartość; poprzednia jest zachowana do `restore_previous`.
    fn set(&self, content: ClipboardContent) -> Result<(), PlatformError>;

    /// Przywraca zawartość sprzed ostatniego `set` (jeśli była).
    fn restore_previous(&self) -> Result<bool, PlatformError>;
}
