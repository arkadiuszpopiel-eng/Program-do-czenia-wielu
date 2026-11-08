//! Atrapa kwarantanny pobrań: pliki w pamięci (bajty + znacznik strefy), katalogi oznaczone jako
//! dowiązanie odrzucane, nazwa końcowa bez nadpisania, porzucony zapis znika.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use platform_apps_contract::{
    DownloadError, DownloadSink, DownloadStore, numbered_name, sanitize_file_name, zone_identifier,
};

/// Plik w kwarantannie atrapy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFile {
    /// Treść.
    pub bytes: Vec<u8>,
    /// Treść znacznika `Zone.Identifier`.
    pub zone: String,
}

#[derive(Debug, Default)]
struct State {
    files: BTreeMap<PathBuf, StoredFile>,
    links: BTreeSet<PathBuf>,
    open: usize,
    aborted: usize,
}

/// Atrapa `DownloadStore`.
#[derive(Debug, Clone, Default)]
pub struct FakeDownloads {
    state: Arc<Mutex<State>>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|p| p.into_inner())
}

impl FakeDownloads {
    /// Oznacza katalog jako dowiązanie/junction (zapis w nim i pod nim → `Unsafe`).
    pub fn mark_link(&self, dir: &Path) {
        lock(&self.state).links.insert(dir.to_path_buf());
    }

    /// Zapisane pliki.
    pub fn files(&self) -> BTreeMap<PathBuf, StoredFile> {
        lock(&self.state).files.clone()
    }

    /// Dodaje istniejący plik (kolizja nazw).
    pub fn put(&self, path: &Path, bytes: &[u8]) {
        lock(&self.state).files.insert(
            path.to_path_buf(),
            StoredFile {
                bytes: bytes.to_vec(),
                zone: String::new(),
            },
        );
    }

    /// Zapisy w toku (niezatwierdzone i nieporzucone).
    pub fn open_sinks(&self) -> usize {
        lock(&self.state).open
    }

    /// Zapisy porzucone (plik częściowy usunięty).
    pub fn aborted(&self) -> usize {
        lock(&self.state).aborted
    }
}

impl DownloadStore for FakeDownloads {
    fn begin(&self, dir: &Path, name: &str) -> Result<Box<dyn DownloadSink>, DownloadError> {
        if name != sanitize_file_name(name) {
            return Err(DownloadError::Unsafe(format!("nazwa „{name}”")));
        }
        let mut s = lock(&self.state);
        if s.links.iter().any(|l| dir.starts_with(l)) {
            return Err(DownloadError::Unsafe(format!(
                "{} prowadzi przez dowiązanie",
                dir.display()
            )));
        }
        s.open += 1;
        Ok(Box::new(FakeSink {
            state: self.state.clone(),
            dir: dir.to_path_buf(),
            name: name.to_owned(),
            bytes: Vec::new(),
            done: false,
        }))
    }
}

struct FakeSink {
    state: Arc<Mutex<State>>,
    dir: PathBuf,
    name: String,
    bytes: Vec<u8>,
    done: bool,
}

impl DownloadSink for FakeSink {
    fn write(&mut self, chunk: &[u8]) -> Result<(), DownloadError> {
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }

    fn commit(mut self: Box<Self>, source_url: &str) -> Result<PathBuf, DownloadError> {
        let mut s = lock(&self.state);
        let mut path = self.dir.join(&self.name);
        let mut n = 2;
        while s.files.contains_key(&path) {
            path = self.dir.join(numbered_name(&self.name, n));
            n += 1;
        }
        s.files.insert(
            path.clone(),
            StoredFile {
                bytes: std::mem::take(&mut self.bytes),
                zone: zone_identifier(source_url),
            },
        );
        s.open = s.open.saturating_sub(1);
        self.done = true;
        Ok(path)
    }
}

impl Drop for FakeSink {
    fn drop(&mut self) {
        if !self.done {
            let mut s = lock(&self.state);
            s.open = s.open.saturating_sub(1);
            s.aborted += 1;
        }
    }
}
