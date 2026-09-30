//! Pliki bazy na dysku: plik główny i pliki poboczne SQLite (`-wal`, `-shm`, `-journal`).

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Sufiksy plików pobocznych SQLite, które muszą zniknąć razem z bazą.
const SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

/// Wszystkie ścieżki należące do bazy `path` (główna + poboczne), istniejące lub nie.
pub fn database_files(path: &Path) -> Vec<PathBuf> {
    let mut out = vec![path.to_path_buf()];
    for suffix in SIDECARS {
        let mut side = path.as_os_str().to_owned();
        side.push(suffix);
        out.push(PathBuf::from(side));
    }
    out
}

/// Usuwa plik bazy razem z `-wal`, `-shm` i `-journal` (crypto-shredding: plik + klucz).
///
/// Idempotentne: brak pliku nie jest błędem. Zwraca listę faktycznie usuniętych plików.
/// Połączenie z bazą musi być wcześniej zamknięte (na Windows otwarty plik nie da się usunąć).
pub fn remove_database(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for file in database_files(path) {
        match std::fs::remove_file(&file) {
            Ok(()) => removed.push(file),
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(removed)
}
