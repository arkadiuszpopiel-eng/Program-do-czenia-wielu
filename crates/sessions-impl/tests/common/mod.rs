//! Wspólne narzędzia testów `sessions-impl`.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::ops::Deref;
use std::sync::Arc;

use sessions_fake::MemoryKeyVault;
use sessions_impl::{SessionsConfig, SqliteSessions};
use tempfile::TempDir;

/// Moduł w katalogu tymczasowym z sejfem w pamięci.
pub struct Harness {
    pub dir: TempDir,
    pub vault: Arc<MemoryKeyVault>,
    pub sessions: SqliteSessions,
}

impl Deref for Harness {
    type Target = SqliteSessions;

    fn deref(&self) -> &SqliteSessions {
        &self.sessions
    }
}

pub fn config(dir: &TempDir) -> SessionsConfig {
    SessionsConfig {
        data_dir: dir.path().join("data"),
        workdir_root: dir.path().join("Sesje"),
    }
}

pub fn harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryKeyVault::new());
    let sessions = SqliteSessions::open(config(&dir), vault.clone()).unwrap();
    Harness {
        dir,
        vault,
        sessions,
    }
}

/// Czy bajty pliku (i jego `-wal`) zawierają tekst jawnie.
pub fn file_contains(path: &std::path::Path, needle: &str) -> bool {
    lib_sqlstore::database_files(path)
        .iter()
        .filter_map(|p| std::fs::read(p).ok())
        .any(|bytes| bytes.windows(needle.len()).any(|w| w == needle.as_bytes()))
}
