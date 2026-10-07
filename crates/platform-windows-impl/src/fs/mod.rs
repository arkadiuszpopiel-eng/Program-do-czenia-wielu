//! `FsPort`: operacje plikowe z deny-listą przed każdą operacją, zapisem atomowym,
//! Koszem (`IFileOperation`) i dziennikiem cofnięć. Część przenośna działa na każdym OS
//! (testy na Linux/CI); Kosz tylko na Windows (gdzie indziej `Unsupported`).

mod guard;
mod journal;
#[cfg(windows)]
mod native;
mod ops;
#[cfg(windows)]
mod recycle;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use platform_contract::{
    DirEntry, FsOperation, FsPort, KnownFolder, OpReceipt, PlatformError, UndoToken,
};

pub use guard::{DEFAULT_EXTRA_DENY_NAMES, DEFAULT_EXTRA_DENY_PREFIXES};

use crate::error::from_io;
pub(crate) use guard::DenyPolicy;
use guard::Follow;
use journal::{Journal, UndoAction};

/// Konfiguracja `WinFs` (`[platform]` w konfiguracji; deny-lista to `kernel_policy`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FsConfig {
    /// Katalog bazowy na kopie zapasowe do cofania `write_atomic` (domyślnie `%TEMP%\alfa-undo`).
    /// Instancja tworzy w nim własny podkatalog i usuwa go przy zamknięciu.
    pub undo_dir: Option<PathBuf>,
    /// Dodatkowe nazwy segmentów na deny-liście (ponad listę kontraktu).
    pub extra_deny_names: Vec<String>,
    /// Dodatkowe prefiksy na deny-liście (mogą zawierać `%ZMIENNE%`).
    pub extra_deny_prefixes: Vec<PathBuf>,
}

impl Default for FsConfig {
    fn default() -> Self {
        Self {
            undo_dir: None,
            extra_deny_names: DEFAULT_EXTRA_DENY_NAMES.map(String::from).to_vec(),
            extra_deny_prefixes: DEFAULT_EXTRA_DENY_PREFIXES.map(PathBuf::from).to_vec(),
        }
    }
}

/// System plików Windows (przenośny poza Koszem).
#[derive(Debug)]
pub struct WinFs {
    policy: DenyPolicy,
    undo_dir: PathBuf,
    journal: Mutex<Journal>,
}

impl WinFs {
    /// Nowy port FS.
    pub fn new(config: FsConfig) -> Self {
        let base = config
            .undo_dir
            .unwrap_or_else(|| std::env::temp_dir().join("alfa-undo"));
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        Self {
            policy: DenyPolicy::new(&config.extra_deny_names, &config.extra_deny_prefixes),
            undo_dir: base.join(format!("{}-{nanos}", std::process::id())),
            journal: Mutex::new(Journal::default()),
        }
    }

    /// Czy ścieżka jest na deny-liście (bez dotykania dysku poza rozwiązaniem ścieżki).
    pub fn is_denied(&self, path: &Path) -> bool {
        matches!(
            self.policy.resolve(path, Follow::NoFinal),
            Err(PlatformError::Denylisted(_))
        )
    }

    /// Polityka deny-listy (współdzielona ze schowkiem: listy plików).
    pub(crate) fn policy(&self) -> &DenyPolicy {
        &self.policy
    }

    /// Liczba oczekujących tokenów cofnięcia.
    pub fn pending_undo(&self) -> usize {
        self.lock().len()
    }

    fn lock(&self) -> MutexGuard<'_, Journal> {
        self.journal.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn record(&self, token: u64, op: FsOperation, action: UndoAction) -> OpReceipt {
        self.lock().insert(token, action);
        OpReceipt::reversible(op, UndoToken(token))
    }

    fn backup(&self, token: u64, source: &Path, user: &Path) -> Result<PathBuf, PlatformError> {
        fs::create_dir_all(&self.undo_dir).map_err(|e| from_io(&e, &self.undo_dir))?;
        let backup = self.undo_dir.join(format!("{token}.bak"));
        fs::copy(source, &backup).map_err(|e| from_io(&e, user))?;
        Ok(backup)
    }
}

impl Default for WinFs {
    fn default() -> Self {
        Self::new(FsConfig::default())
    }
}

impl Drop for WinFs {
    fn drop(&mut self) {
        let journal = self.journal.get_mut().unwrap_or_else(|p| p.into_inner());
        for backup in journal.backups() {
            let _ = fs::remove_file(backup);
        }
        let _ = fs::remove_dir(&self.undo_dir);
    }
}

fn not_found_unless_exists(op: &Path, user: &Path) -> Result<(), PlatformError> {
    fs::symlink_metadata(op)
        .map(|_| ())
        .map_err(|_| PlatformError::NotFound(user.to_path_buf()))
}

impl FsPort for WinFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PlatformError> {
        let op = self.policy.resolve(path, Follow::Final)?;
        fs::read(&op).map_err(|e| from_io(&e, path))
    }

    fn write_atomic(&self, path: &Path, data: &[u8]) -> Result<OpReceipt, PlatformError> {
        let op = self.policy.resolve(path, Follow::Final)?;
        // Zapis przez dowiązanie trafia do celu (sprawdzonego przez deny-listę w `resolve`).
        let op = if op.is_symlink() {
            fs::canonicalize(&op).map_err(|e| from_io(&e, path))?
        } else {
            op
        };
        if op.is_dir() {
            return Err(PlatformError::InvalidPath(path.to_path_buf()));
        }
        let token = self.lock().allocate();
        let created_dirs = ops::create_parents(&op).map_err(|e| from_io(&e, path))?;
        let previous_modified = fs::metadata(&op).and_then(|m| m.modified()).ok();
        let backup = if op.exists() {
            Some(self.backup(token, &op, path)?)
        } else {
            None
        };
        if let Err(e) = ops::write_atomic_file(&op, data) {
            if let Some(b) = &backup {
                let _ = fs::remove_file(b);
            }
            ops::remove_created_dirs(&created_dirs);
            return Err(from_io(&e, path));
        }
        let after = ops::fingerprint(&op);
        let action = UndoAction::RestoreContent {
            path: op,
            backup,
            previous_modified,
            created_dirs,
            after,
        };
        Ok(self.record(token, FsOperation::WriteAtomic, action))
    }

    fn copy(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError> {
        let src = self.policy.resolve(from, Follow::Final)?;
        let dst = self.policy.resolve(to, Follow::NoFinal)?;
        if !src.exists() {
            return Err(PlatformError::NotFound(from.to_path_buf()));
        }
        if fs::symlink_metadata(&dst).is_ok() {
            return Err(PlatformError::AlreadyExists(to.to_path_buf()));
        }
        if dst.starts_with(&src) {
            // Kopia katalogu do własnego wnętrza rosłaby bez końca.
            return Err(PlatformError::InvalidPath(to.to_path_buf()));
        }
        let created_dirs = ops::create_parents(&dst).map_err(|e| from_io(&e, to))?;
        if let Err(e) = ops::copy_entry(&src, &dst, &self.policy, from) {
            ops::remove_created_dirs(&created_dirs);
            return Err(e);
        }
        let token = self.lock().allocate();
        let after = ops::fingerprint(&dst);
        let action = UndoAction::RemoveCopy {
            path: dst,
            created_dirs,
            after,
        };
        Ok(self.record(token, FsOperation::Copy, action))
    }

    fn move_path(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError> {
        let src = self.policy.resolve(from, Follow::NoFinal)?;
        let dst = self.policy.resolve(to, Follow::NoFinal)?;
        not_found_unless_exists(&src, from)?;
        let token = self.lock().allocate();
        if src == dst {
            return Ok(self.record(token, FsOperation::Move, UndoAction::Noop));
        }
        if fs::symlink_metadata(&dst).is_ok() && !ops::same_entry(&src, &dst) {
            return Err(PlatformError::AlreadyExists(to.to_path_buf()));
        }
        let created_dirs = ops::create_parents(&dst).map_err(|e| from_io(&e, to))?;
        if let Err(e) = ops::move_no_replace(&src, &dst) {
            ops::remove_created_dirs(&created_dirs);
            return Err(from_io(&e, to));
        }
        let action = UndoAction::MoveBack {
            current: dst,
            original: src,
            created_dirs,
        };
        Ok(self.record(token, FsOperation::Move, action))
    }

    fn delete_to_recycle_bin(&self, path: &Path) -> Result<OpReceipt, PlatformError> {
        let op = self.policy.resolve(path, Follow::NoFinal)?;
        not_found_unless_exists(&op, path)?;
        #[cfg(windows)]
        {
            match recycle::recycle(&op)? {
                recycle::RecycleOutcome::Recycled(recycled) => {
                    let token = self.lock().allocate();
                    let action = UndoAction::RestoreFromBin {
                        recycled,
                        original: op,
                    };
                    Ok(self.record(token, FsOperation::DeleteToRecycleBin, action))
                }
                // Kosz niedostępny, użytkownik potwierdził trwałe usunięcie: pokwitowanie mówi prawdę.
                recycle::RecycleOutcome::Destroyed => {
                    Ok(OpReceipt::irreversible(FsOperation::DeletePermanent))
                }
            }
        }
        #[cfg(not(windows))]
        {
            Err(PlatformError::Unsupported(format!(
                "{}: Kosz jest dostępny tylko na Windows",
                path.display()
            )))
        }
    }

    fn delete_permanent(&self, path: &Path) -> Result<OpReceipt, PlatformError> {
        let op = self.policy.resolve(path, Follow::NoFinal)?;
        not_found_unless_exists(&op, path)?;
        ops::remove_entry(&op).map_err(|e| from_io(&e, path))?;
        Ok(OpReceipt::irreversible(FsOperation::DeletePermanent))
    }

    fn exists(&self, path: &Path) -> bool {
        self.policy
            .resolve(path, Follow::Final)
            .is_ok_and(|op| op.exists())
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        let op = self.policy.resolve(path, Follow::Final)?;
        if op.exists() && !op.is_dir() {
            return Err(PlatformError::InvalidPath(path.to_path_buf()));
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(&op).map_err(|e| from_io(&e, path))? {
            let entry = entry.map_err(|e| from_io(&e, path))?;
            let meta = fs::metadata(entry.path()).or_else(|_| entry.metadata());
            let (is_dir, size) = meta.map_or((false, 0), |m| {
                (m.is_dir(), if m.is_dir() { 0 } else { m.len() })
            });
            entries.push(DirEntry {
                path: path.join(entry.file_name()),
                is_dir,
                size,
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(entries)
    }

    fn undo(&self, token: UndoToken) -> Result<(), PlatformError> {
        let action = self.lock().take(token)?;
        journal::apply(&action).inspect_err(|_| self.lock().insert(token.0, action))
    }

    fn known_folder(&self, folder: KnownFolder) -> PathBuf {
        #[cfg(windows)]
        if let Some(path) = native::known_folder(folder) {
            return path;
        }
        fallback_known_folder(folder)
    }
}

/// Znane foldery z ustawień środowiska (poza Windows lub gdy powłoka odmówi).
fn fallback_known_folder(folder: KnownFolder) -> PathBuf {
    let env = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let home = env("USERPROFILE")
        .or_else(|| env("HOME"))
        .unwrap_or_else(std::env::temp_dir);
    match folder {
        KnownFolder::LocalAppData => env("LOCALAPPDATA")
            .or_else(|| env("XDG_DATA_HOME"))
            .unwrap_or_else(|| home.join(".local").join("share")),
        KnownFolder::RoamingAppData => env("APPDATA")
            .or_else(|| env("XDG_CONFIG_HOME"))
            .unwrap_or_else(|| home.join(".config")),
        KnownFolder::Home => home,
        KnownFolder::Temp => std::env::temp_dir(),
    }
}
