//! Wirtualny system plików z dziennikiem cofnięć.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use platform_contract::{
    DirEntry, FsOperation, FsPort, KnownFolder, OpReceipt, PlatformError, UndoToken,
    is_credential_path,
};

/// Migawka zawartości (do porównań w testach).
pub type FsSnapshot = BTreeMap<PathBuf, Vec<u8>>;

#[derive(Debug)]
enum UndoAction {
    /// Przywróć poprzednią zawartość (`None` = plik nie istniał).
    RestoreContent {
        path: PathBuf,
        previous: Option<Vec<u8>>,
    },
    /// Usuń plik (cofnięcie kopii).
    Remove(PathBuf),
    /// Przenieś z powrotem.
    MoveBack { from: PathBuf, to: PathBuf },
    /// Przywróć z Kosza.
    RestoreFromBin { path: PathBuf, content: Vec<u8> },
}

#[derive(Debug, Default)]
struct State {
    files: FsSnapshot,
    journal: BTreeMap<u64, UndoAction>,
    next_token: u64,
    recycle_bin: Vec<(PathBuf, Vec<u8>)>,
}

/// Wirtualny FS: model plikowy (katalogi wynikają z prefiksów ścieżek), bez we/wy.
#[derive(Debug, Default)]
pub struct FakeFs {
    state: Mutex<State>,
}

impl FakeFs {
    /// Pusty FS.
    pub fn new() -> Self {
        Self::default()
    }

    /// FS z początkową zawartością.
    pub fn with_files(files: impl IntoIterator<Item = (PathBuf, Vec<u8>)>) -> Self {
        let fs = Self::new();
        fs.lock().files.extend(files);
        fs
    }

    /// Migawka wszystkich plików.
    pub fn snapshot(&self) -> FsSnapshot {
        self.lock().files.clone()
    }

    /// Zawartość Kosza (ścieżka, dane) w kolejności usuwania.
    pub fn recycle_bin(&self) -> Vec<(PathBuf, Vec<u8>)> {
        self.lock().recycle_bin.clone()
    }

    /// Liczba tokenów cofnięcia oczekujących w dzienniku.
    pub fn pending_undo(&self) -> usize {
        self.lock().journal.len()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn check_path(path: &Path) -> Result<(), PlatformError> {
    if path.as_os_str().is_empty() {
        return Err(PlatformError::InvalidPath(path.to_path_buf()));
    }
    if is_credential_path(path) {
        return Err(PlatformError::Denylisted(path.to_path_buf()));
    }
    Ok(())
}

impl State {
    fn record(&mut self, op: FsOperation, action: UndoAction) -> OpReceipt {
        self.next_token += 1;
        let token = UndoToken(self.next_token);
        self.journal.insert(token.0, action);
        OpReceipt::reversible(op, token)
    }

    fn take(&self, path: &Path) -> Result<Vec<u8>, PlatformError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| PlatformError::NotFound(path.to_path_buf()))
    }

    fn ensure_absent(&self, path: &Path) -> Result<(), PlatformError> {
        if self.files.contains_key(path) {
            return Err(PlatformError::AlreadyExists(path.to_path_buf()));
        }
        Ok(())
    }
}

impl FsPort for FakeFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PlatformError> {
        check_path(path)?;
        self.lock().take(path)
    }

    fn write_atomic(&self, path: &Path, data: &[u8]) -> Result<OpReceipt, PlatformError> {
        check_path(path)?;
        let mut st = self.lock();
        let previous = st.files.insert(path.to_path_buf(), data.to_vec());
        Ok(st.record(
            FsOperation::WriteAtomic,
            UndoAction::RestoreContent {
                path: path.to_path_buf(),
                previous,
            },
        ))
    }

    fn copy(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError> {
        check_path(from)?;
        check_path(to)?;
        let mut st = self.lock();
        let content = st.take(from)?;
        st.ensure_absent(to)?;
        st.files.insert(to.to_path_buf(), content);
        Ok(st.record(FsOperation::Copy, UndoAction::Remove(to.to_path_buf())))
    }

    fn move_path(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError> {
        check_path(from)?;
        check_path(to)?;
        let mut st = self.lock();
        let content = st.take(from)?;
        if from != to {
            st.ensure_absent(to)?;
        }
        st.files.remove(from);
        st.files.insert(to.to_path_buf(), content);
        Ok(st.record(
            FsOperation::Move,
            UndoAction::MoveBack {
                from: to.to_path_buf(),
                to: from.to_path_buf(),
            },
        ))
    }

    fn delete_to_recycle_bin(&self, path: &Path) -> Result<OpReceipt, PlatformError> {
        check_path(path)?;
        let mut st = self.lock();
        let content = st.take(path)?;
        st.files.remove(path);
        st.recycle_bin.push((path.to_path_buf(), content.clone()));
        Ok(st.record(
            FsOperation::DeleteToRecycleBin,
            UndoAction::RestoreFromBin {
                path: path.to_path_buf(),
                content,
            },
        ))
    }

    fn delete_permanent(&self, path: &Path) -> Result<OpReceipt, PlatformError> {
        check_path(path)?;
        let mut st = self.lock();
        st.take(path)?;
        st.files.remove(path);
        Ok(OpReceipt::irreversible(FsOperation::DeletePermanent))
    }

    fn exists(&self, path: &Path) -> bool {
        let st = self.lock();
        st.files.contains_key(path) || st.files.keys().any(|k| k.starts_with(path) && k != path)
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        check_path(path)?;
        let st = self.lock();
        let mut entries: BTreeMap<PathBuf, DirEntry> = BTreeMap::new();
        for (file, data) in &st.files {
            let Ok(rest) = file.strip_prefix(path) else {
                continue;
            };
            let Some(first) = rest.components().next() else {
                continue;
            };
            let child = path.join(first.as_os_str());
            let is_dir = rest.components().count() > 1;
            let size = if is_dir { 0 } else { data.len() as u64 };
            entries.entry(child.clone()).or_insert(DirEntry {
                path: child,
                is_dir,
                size,
            });
        }
        if entries.is_empty() && !st.files.contains_key(path) {
            return Err(PlatformError::NotFound(path.to_path_buf()));
        }
        Ok(entries.into_values().collect())
    }

    fn undo(&self, token: UndoToken) -> Result<(), PlatformError> {
        let mut st = self.lock();
        let action = st
            .journal
            .remove(&token.0)
            .ok_or(PlatformError::UnknownUndoToken(token.0))?;
        match action {
            UndoAction::RestoreContent { path, previous } => match previous {
                Some(data) => {
                    st.files.insert(path, data);
                }
                None => {
                    st.files.remove(&path);
                }
            },
            UndoAction::Remove(path) => {
                st.files.remove(&path);
            }
            UndoAction::MoveBack { from, to } => {
                let content = st.take(&from)?;
                st.files.remove(&from);
                st.files.insert(to, content);
            }
            UndoAction::RestoreFromBin { path, content } => {
                if let Some(pos) = st.recycle_bin.iter().rposition(|(p, _)| *p == path) {
                    st.recycle_bin.remove(pos);
                }
                st.files.insert(path, content);
            }
        }
        Ok(())
    }

    fn known_folder(&self, folder: KnownFolder) -> PathBuf {
        PathBuf::from(match folder {
            KnownFolder::LocalAppData => "/fake/AppData/Local",
            KnownFolder::RoamingAppData => "/fake/AppData/Roaming",
            KnownFolder::Home => "/fake/home",
            KnownFolder::Temp => "/fake/tmp",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denylist_and_missing() {
        let fs = FakeFs::new();
        assert_eq!(
            fs.read(Path::new("/home/u/.codex/auth.json")),
            Err(PlatformError::Denylisted("/home/u/.codex/auth.json".into()))
        );
        assert_eq!(
            fs.read(Path::new("/x")),
            Err(PlatformError::NotFound("/x".into()))
        );
        assert_eq!(
            fs.write_atomic(Path::new(""), b"").unwrap_err(),
            PlatformError::InvalidPath("".into())
        );
    }

    #[test]
    fn list_dir_shows_files_and_implied_dirs() {
        let fs = FakeFs::with_files([
            ("/d/a.txt".into(), b"aa".to_vec()),
            ("/d/sub/b.txt".into(), b"b".to_vec()),
        ]);
        let entries = fs.list_dir(Path::new("/d")).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .any(|e| e.path == Path::new("/d/a.txt") && !e.is_dir && e.size == 2)
        );
        assert!(
            entries
                .iter()
                .any(|e| e.path == Path::new("/d/sub") && e.is_dir)
        );
        assert!(fs.exists(Path::new("/d/sub")));
        assert!(fs.list_dir(Path::new("/nope")).is_err());
    }

    #[test]
    fn permanent_delete_has_no_undo_and_tokens_are_single_use() {
        let fs = FakeFs::with_files([("/f".into(), b"1".to_vec())]);
        let r = fs.delete_permanent(Path::new("/f")).unwrap();
        assert!(!r.reversible && r.undo.is_none());
        let w = fs.write_atomic(Path::new("/g"), b"x").unwrap();
        let token = w.undo.unwrap();
        fs.undo(token).unwrap();
        assert_eq!(
            fs.undo(token),
            Err(PlatformError::UnknownUndoToken(token.0))
        );
        assert!(!fs.exists(Path::new("/g")));
    }
}
