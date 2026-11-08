//! Port procesów.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Poziom integralności procesu potomnego (Windows: restricted token / low / AppContainer).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Integrity {
    /// Jak proces jądra.
    #[default]
    Medium,
    /// Niska integralność.
    Low,
    /// AppContainer (izolacja dla niezaufanych, ≤ L3).
    AppContainer,
}

/// Specyfikacja uruchomienia procesu (zawsze w Job Object, PLAN §3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessSpec {
    /// Ścieżka programu.
    pub cmd: PathBuf,
    /// Argumenty.
    #[serde(default)]
    pub args: Vec<String>,
    /// Katalog roboczy.
    pub cwd: PathBuf,
    /// Integralność.
    #[serde(default)]
    pub integrity: Integrity,
    /// Limit pamięci drzewa procesów w MB (Job Object), jeśli ustawiony.
    #[serde(default)]
    pub memory_limit_mb: Option<u32>,
}

/// Uchwyt procesu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessHandle(pub u32);

/// Stan procesu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessStatus {
    /// Działa.
    Running,
    /// Zakończony z kodem.
    Exited(i32),
    /// Zabity przez `kill_tree`.
    Killed,
}

/// Port procesów.
pub trait ProcessPort: Send + Sync {
    /// Uruchamia proces w Job Object.
    fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, PlatformError>;

    /// Zabija całe drzewo procesów.
    fn kill_tree(&self, handle: ProcessHandle) -> Result<(), PlatformError>;

    /// Stan procesu.
    fn status(&self, handle: ProcessHandle) -> Result<ProcessStatus, PlatformError>;

    /// Czy okno na pierwszym planie należy do procesu podniesionego (hooki nie działają, PLAN §7.3).
    fn foreground_is_elevated(&self) -> bool;
}
