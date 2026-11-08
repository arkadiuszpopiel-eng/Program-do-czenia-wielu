//! Kontekst planisty na dysku (`DirContext`): kopie zapasowe, kwarantanna, archiwum, korzenie
//! Jądra, wolne porty (próba `bind` na 127.0.0.1), modele i katalogi zastępcze, widok konfiguracji.

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use diagnostician_contract::RepairContext;
use serde_json::Value;
use watchdog_contract::{Clock, ConfigHistory};

/// Widok konfiguracji (synchroniczny — np. migawka utrzymywana przez kompozycję).
pub type ConfigView = Arc<dyn Fn(&str) -> Option<Value> + Send + Sync>;
/// Ostatnia dobra rewizja konfiguracji.
pub type LastGood = Arc<dyn Fn() -> Option<String> + Send + Sync>;
/// Pliki do przeniesienia z woluminu: (skąd, dokąd).
pub type Reclaim = Arc<dyn Fn(&str) -> Vec<(String, String)> + Send + Sync>;

/// Katalogi Diagnosty.
#[derive(Debug, Clone)]
pub struct DiagDirs {
    /// Korzeń danych Alfy (np. `%LOCALAPPDATA%\Alfa`).
    pub data_root: PathBuf,
    /// Kopie zapasowe (odwzorowanie ścieżek względem `data_root`).
    pub backups: PathBuf,
    /// Kwarantanna.
    pub quarantine: PathBuf,
    /// Archiwum wpisów.
    pub archive: PathBuf,
}

/// Kontekst planisty.
pub struct DirContext {
    clock: Arc<dyn Clock>,
    dirs: DiagDirs,
    kernel_roots: Vec<String>,
    config: Option<ConfigView>,
    history: Option<(Arc<dyn ConfigHistory>, LastGood)>,
    fallback_models: BTreeMap<String, String>,
    fallback_dir: Option<PathBuf>,
    reclaim: Option<Reclaim>,
    seq: AtomicU64,
}

fn norm(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

impl DirContext {
    /// Kontekst z katalogami i korzeniami Jądra (instalacja, dane Brokera, polityki).
    pub fn new(clock: Arc<dyn Clock>, dirs: DiagDirs, kernel_roots: &[PathBuf]) -> Self {
        Self {
            clock,
            dirs,
            kernel_roots: kernel_roots
                .iter()
                .map(|p| norm(&p.to_string_lossy()))
                .collect(),
            config: None,
            history: None,
            fallback_models: BTreeMap::new(),
            fallback_dir: None,
            reclaim: None,
            seq: AtomicU64::new(0),
        }
    }

    /// Widok konfiguracji (builder).
    #[must_use]
    pub fn with_config_view(mut self, view: ConfigView) -> Self {
        self.config = Some(view);
        self
    }

    /// Rewizje konfiguracji (builder).
    #[must_use]
    pub fn with_revisions(mut self, history: Arc<dyn ConfigHistory>, last_good: LastGood) -> Self {
        self.history = Some((history, last_good));
        self
    }

    /// Modele zastępcze (builder).
    #[must_use]
    pub fn with_fallback_models(mut self, models: BTreeMap<String, String>) -> Self {
        self.fallback_models = models;
        self
    }

    /// Katalog zastępczy z prawem zapisu (builder).
    #[must_use]
    pub fn with_fallback_dir(mut self, dir: PathBuf) -> Self {
        self.fallback_dir = Some(dir);
        self
    }

    /// Źródło plików do przeniesienia przy braku miejsca (builder).
    #[must_use]
    pub fn with_reclaimable(mut self, reclaim: Reclaim) -> Self {
        self.reclaim = Some(reclaim);
        self
    }

    fn relative<'a>(&self, path: &'a str) -> Option<&'a Path> {
        Path::new(path).strip_prefix(&self.dirs.data_root).ok()
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(|| "plik".into(), |n| n.to_string_lossy().into_owned())
}

impl RepairContext for DirContext {
    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    fn config(&self, key: &str) -> Option<Value> {
        self.config.as_ref().and_then(|view| view(key))
    }

    fn current_revision(&self) -> Option<String> {
        self.history
            .as_ref()
            .and_then(|(h, _)| h.current_revision())
    }

    fn last_good_revision(&self) -> Option<String> {
        self.history.as_ref().and_then(|(_, good)| good())
    }

    fn latest_backup(&self, path: &str) -> Option<String> {
        let backup = self.dirs.backups.join(self.relative(path)?);
        backup
            .is_file()
            .then(|| backup.to_string_lossy().into_owned())
    }

    fn quarantine_path(&self, path: &str) -> String {
        let n = self.seq.fetch_add(1, Ordering::SeqCst);
        let dir = format!("{}-{n}", self.clock.now_ms());
        self.dirs
            .quarantine
            .join(dir)
            .join(file_name(path))
            .to_string_lossy()
            .into_owned()
    }

    fn free_port(&self, near: u16) -> Option<u16> {
        (near.saturating_add(1)..near.saturating_add(200))
            .find(|p| TcpListener::bind(("127.0.0.1", *p)).is_ok())
    }

    fn fallback_model(&self, model: &str) -> Option<String> {
        self.fallback_models.get(model).cloned()
    }

    fn fallback_dir(&self, _: &str) -> Option<String> {
        self.fallback_dir
            .as_ref()
            .map(|d| d.to_string_lossy().into_owned())
    }

    fn archive_path(&self, store: &str) -> String {
        let safe: String = store
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.dirs.archive.join(safe).to_string_lossy().into_owned()
    }

    fn reclaimable(&self, volume: &str) -> Vec<(String, String)> {
        self.reclaim.as_ref().map(|r| r(volume)).unwrap_or_default()
    }

    fn is_kernel_path(&self, path: &str) -> bool {
        let p = norm(path);
        self.kernel_roots
            .iter()
            .any(|root| p == *root || p.starts_with(&format!("{}/", root.trim_end_matches('/'))))
    }
}
