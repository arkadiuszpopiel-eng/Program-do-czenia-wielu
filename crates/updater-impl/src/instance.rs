//! Blokada działającej instancji (`<root>\running.lock`, `File::try_lock` — zwalniana przez
//! system przy zakończeniu procesu, także po awarii). Aplikacja trzyma ją przez cały czas
//! życia; launcher w trybie `--alfa-restart` czeka, aż stara instancja ją zwolni, zanim
//! uruchomi nową wersję (inaczej druga instancja oddałaby argumenty starej i zakończyła się).

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use crate::launcher::LaunchClock;

/// Plik blokady w katalogu instalacji.
pub const RUNNING_LOCK: &str = "running.lock";

/// Blokada trzymana przez działającą aplikację.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
    path: PathBuf,
}

fn open(path: &Path) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
}

impl InstanceLock {
    /// Zakłada blokadę; `Ok(None)` — trzyma ją inny proces.
    pub fn acquire(root: &Path) -> std::io::Result<Option<Self>> {
        let path = root.join(RUNNING_LOCK);
        let file = open(&path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file, path })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(e)) => Err(e),
        }
    }

    /// Ścieżka pliku blokady.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Czeka (do `timeout_ms`), aż nikt nie trzyma blokady; `true` — wolna.
pub fn wait_released(root: &Path, clock: &dyn LaunchClock, timeout_ms: u64) -> bool {
    let path = root.join(RUNNING_LOCK);
    let Ok(file) = open(&path) else {
        return true;
    };
    let start = clock.now_ms();
    loop {
        match file.try_lock() {
            Ok(()) => {
                let _ = file.unlock();
                return true;
            }
            Err(TryLockError::Error(_)) => return true,
            Err(TryLockError::WouldBlock) => {}
        }
        if clock.now_ms().saturating_sub(start) >= timeout_ms {
            return false;
        }
        clock.sleep_ms(100);
    }
}
