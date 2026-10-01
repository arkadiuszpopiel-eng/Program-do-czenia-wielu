//! Pliki tymczasowe obok celu (ten sam wolumin → atomowa zamiana przez `rename`), usuwane przy
//! zwolnieniu, jeśli nie zostały utrwalone.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use transfer_contract::{TransferError, hex};

/// Przyrostek plików tymczasowych modułu (sprzątanie po awarii: [`sweep`]).
pub const TEMP_SUFFIX: &str = ".alfa-tmp";

/// Plik tymczasowy.
pub struct TempFile {
    path: PathBuf,
    keep: bool,
}

impl TempFile {
    /// Nowa (jeszcze nieistniejąca) ścieżka `.<nazwa celu>.<losowe>.alfa-tmp` w `dir`.
    pub fn new(dir: &Path, dest: &Path) -> Result<Self, TransferError> {
        let mut rnd = [0u8; 8];
        getrandom::fill(&mut rnd).map_err(|e| TransferError::io(format!("losowość: {e}")))?;
        let stem = dest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self {
            path: dir.join(format!(".{stem}.{}{TEMP_SUFFIX}", hex(&rnd))),
            keep: false,
        })
    }

    /// Ścieżka.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Tworzy plik (nie nadpisuje istniejącego).
    pub fn create(&self) -> Result<File, TransferError> {
        Ok(OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .open(&self.path)?)
    }

    /// Atomowo zastępuje `dest` tym plikiem; zwraca rozmiar.
    pub fn persist(mut self, dest: &Path) -> Result<u64, TransferError> {
        std::fs::rename(&self.path, dest)?;
        self.keep = true;
        Ok(std::fs::metadata(dest)?.len())
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Usuwa osierocone pliki tymczasowe modułu w katalogu (po awarii w trakcie zapisu).
pub fn sweep(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(TEMP_SUFFIX))
        .filter(|e| std::fs::remove_file(e.path()).is_ok())
        .count()
}
