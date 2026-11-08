//! Pseudokonsola (ConPTY) dla wbudowanego terminala `ui-terminal` (F4, PLAN §5.5, §5.6):
//! proces w Job Object (zamknięcie zabija całe drzewo), wyjście VT do UI, wejście z klawiatury UI.
//!
//! Port **nie** loguje, nie buforuje na dysku i nie analizuje strumienia (w terminalu logowania
//! do CLI mogą pojawić się tokeny). Środowisko jest jawne — nic nie jest dziedziczone
//! (zob. [`crate::filter_env`]); sekrety Alfy nigdy nie trafiają do procesu terminala.

use std::io::Read;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Rozmiar terminala (kolumny × wiersze).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtySize {
    /// Kolumny (2–1000).
    pub cols: u16,
    /// Wiersze (1–1000).
    pub rows: u16,
}

impl PtySize {
    /// Walidacja zakresu.
    pub fn validate(&self) -> Result<(), PlatformError> {
        if (2..=1000).contains(&self.cols) && (1..=1000).contains(&self.rows) {
            Ok(())
        } else {
            Err(PlatformError::Unsupported(format!(
                "rozmiar terminala {}×{} poza zakresem",
                self.cols, self.rows
            )))
        }
    }
}

impl Default for PtySize {
    fn default() -> Self {
        Self {
            cols: 120,
            rows: 30,
        }
    }
}

/// Specyfikacja procesu w pseudokonsoli.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtySpec {
    /// Program (ścieżka bezwzględna).
    pub program: PathBuf,
    /// Argumenty.
    pub args: Vec<String>,
    /// Katalog roboczy.
    pub cwd: PathBuf,
    /// Pełne środowisko (nic nie jest dziedziczone).
    pub env: Vec<(String, String)>,
    /// Rozmiar początkowy.
    pub size: PtySize,
}

impl PtySpec {
    /// Walidacja wspólna dla implementacji.
    pub fn validate(&self) -> Result<(), PlatformError> {
        if !self.program.is_absolute() {
            return Err(PlatformError::InvalidPath(self.program.clone()));
        }
        if self.args.iter().any(|a| a.contains('\0'))
            || self
                .env
                .iter()
                .any(|(k, v)| k.is_empty() || k.contains(['=', '\0']) || v.contains('\0'))
        {
            return Err(PlatformError::Unsupported(
                "argument albo zmienna środowiskowa z NUL/`=`".into(),
            ));
        }
        self.size.validate()
    }
}

/// Sesja pseudokonsoli. `Debug` implementacji nie może pokazywać treści strumienia.
pub trait PtySession: Send + Sync {
    /// PID procesu głównego.
    fn pid(&self) -> u32;
    /// Strumień wyjścia (VT) — do pobrania **raz**; czytany na osobnym wątku do EOF.
    fn take_output(&self) -> Result<Box<dyn Read + Send>, PlatformError>;
    /// Wejście (bajty z klawiatury UI).
    fn write_input(&self, data: &[u8]) -> Result<(), PlatformError>;
    /// Zmiana rozmiaru.
    fn resize(&self, size: PtySize) -> Result<(), PlatformError>;
    /// Kod wyjścia, jeśli proces się zakończył.
    fn exit_code(&self) -> Result<Option<i32>, PlatformError>;
    /// Zamyka pseudokonsolę i zabija całe drzewo procesów (Job Object). Idempotentne.
    fn close(&self) -> Result<(), PlatformError>;
}

/// Port pseudokonsoli.
pub trait PseudoConsolePort: Send + Sync {
    /// Uruchamia proces w nowej pseudokonsoli i własnym Job Object.
    fn spawn(&self, spec: &PtySpec) -> Result<Box<dyn PtySession>, PlatformError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_validation() {
        let abs = if cfg!(windows) {
            r"C:\w\pwsh.exe"
        } else {
            "/bin/sh"
        };
        let spec = PtySpec {
            program: PathBuf::from(abs),
            args: vec!["-NoLogo".into()],
            cwd: PathBuf::from("/"),
            env: vec![("PATH".into(), "x".into())],
            size: PtySize::default(),
        };
        assert!(spec.validate().is_ok());
        let mut s = spec.clone();
        s.program = PathBuf::from("pwsh.exe");
        assert!(s.validate().is_err());
        let mut s = spec.clone();
        s.env = vec![("A=B".into(), "x".into())];
        assert!(s.validate().is_err());
        let mut s = spec.clone();
        s.args = vec!["a\0".into()];
        assert!(s.validate().is_err());
        let mut s = spec;
        s.size = PtySize { cols: 1, rows: 10 };
        assert!(s.validate().is_err());
    }
}
