//! Adaptery portów naprawy dla kompozycji `app-*`: `PortsEnv` (implementacja `RepairEnv`)
//! składa wąskie porty — `core-config` (porównaj-i-zamień, origin = diagnostician), historię
//! konfiguracji (watchdog), pliki w dozwolonych korzeniach, restart modułów, archiwa wpisów,
//! kolejkę pobrań i sondę zdrowia.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, Origin, Scope};
use diagnostician_contract::{Detection, RepairEnv, RepairStep, is_forbidden_key, is_kernel_key};
use sha2::{Digest, Sha256};
use watchdog_contract::ConfigHistory;

/// Operacje na plikach (zawsze bez utraty danych).
pub trait FilePort: Send + Sync {
    /// Przeniesienie (cel nie może istnieć).
    fn move_path(&self, from: &str, to: &str) -> Result<(), String>;
    /// Kopia (cel nie może istnieć; zweryfikowana SHA-256).
    fn copy_file(&self, from: &str, to: &str) -> Result<(), String>;
    /// Usunięcie kopii identycznej (SHA-256) ze źródłem, które zostaje.
    fn remove_identical_copy(&self, path: &str, source: &str) -> Result<(), String>;
}

fn sha256_file(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Sha256::digest(&bytes).to_vec())
}

/// Pliki lokalne ograniczone do dozwolonych korzeni (dane Alfy: kwarantanna, kopie, archiwum,
/// dane modułów). Ścieżki muszą być bezwzględne, bez `..`, także po rozwiązaniu dowiązań.
#[derive(Debug, Clone)]
pub struct LocalFiles {
    roots: Vec<PathBuf>,
}

impl LocalFiles {
    /// Korzenie muszą istnieć (kanonizowane).
    pub fn new(roots: impl IntoIterator<Item = PathBuf>) -> std::io::Result<Self> {
        let roots = roots
            .into_iter()
            .map(fs::canonicalize)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { roots })
    }

    fn resolve(&self, raw: &str) -> Result<PathBuf, String> {
        let path = PathBuf::from(raw);
        if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(format!("niedozwolona ścieżka `{raw}`"));
        }
        // Najbliższy istniejący przodek po kanonizacji (dowiązania) musi leżeć w korzeniu.
        let mut probe = path.as_path();
        let canonical_ancestor = loop {
            if let Ok(c) = fs::canonicalize(probe) {
                break c;
            }
            probe = probe
                .parent()
                .ok_or_else(|| format!("brak przodka `{raw}`"))?;
        };
        if self.roots.iter().any(|r| canonical_ancestor.starts_with(r)) {
            Ok(path)
        } else {
            Err(format!("`{raw}` poza dozwolonymi katalogami Diagnosty"))
        }
    }

    fn prepare_target(&self, to: &str) -> Result<PathBuf, String> {
        let to = self.resolve(to)?;
        if to.exists() {
            return Err(format!("{} już istnieje", to.display()));
        }
        if let Some(dir) = to.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        Ok(to)
    }
}

impl FilePort for LocalFiles {
    fn move_path(&self, from: &str, to: &str) -> Result<(), String> {
        let from = self.resolve(from)?;
        let to = self.prepare_target(to)?;
        fs::rename(&from, &to).map_err(|e| format!("{} → {}: {e}", from.display(), to.display()))
    }

    fn copy_file(&self, from: &str, to: &str) -> Result<(), String> {
        let from = self.resolve(from)?;
        let to = self.prepare_target(to)?;
        fs::copy(&from, &to).map_err(|e| e.to_string())?;
        if sha256_file(&from)? != sha256_file(&to)? {
            let _ = fs::remove_file(&to);
            return Err("kopia niezgodna ze źródłem".into());
        }
        Ok(())
    }

    fn remove_identical_copy(&self, path: &str, source: &str) -> Result<(), String> {
        let (path, source) = (self.resolve(path)?, self.resolve(source)?);
        if !path.is_file() || !source.is_file() || sha256_file(&path)? != sha256_file(&source)? {
            return Err(format!(
                "{} nie jest identyczną kopią {}",
                path.display(),
                source.display()
            ));
        }
        fs::remove_file(&path).map_err(|e| e.to_string())
    }
}

/// Restart modułu (rejestr / watchdog).
#[async_trait]
pub trait ModuleRestarter: Send + Sync {
    /// Uruchamia moduł ponownie.
    async fn restart(&self, module: &str) -> Result<(), String>;
}

/// Archiwum wpisów (dziennik cofania, segmenty logów).
pub trait EntryStore: Send + Sync {
    /// Przenosi najstarsze wpisy do archiwum.
    fn archive(&self, store: &str, entries: u64, archive: &str) -> Result<(), String>;
    /// Przywraca wpisy z archiwum.
    fn restore(&self, store: &str, entries: u64, archive: &str) -> Result<(), String>;
}

/// Kolejka pobrań (modele) ze sprawdzeniem SHA-256.
pub trait DownloadQueue: Send + Sync {
    /// Dodaje.
    fn queue(&self, item: &str, sha256: &str) -> Result<(), String>;
    /// Anuluje.
    fn cancel(&self, item: &str) -> Result<(), String>;
}

/// Sonda zdrowia po naprawie.
#[async_trait]
pub trait HealthProbe: Send + Sync {
    /// Czy awaria ustąpiła.
    async fn healthy(&self, detection: &Detection) -> Result<bool, String>;
}

/// Środowisko napraw złożone z portów.
pub struct PortsEnv {
    /// Magazyn konfiguracji.
    pub config: Arc<dyn ConfigStore>,
    /// Historia konfiguracji (rewizje).
    pub history: Arc<dyn ConfigHistory>,
    /// Pliki.
    pub files: Arc<dyn FilePort>,
    /// Restart modułów.
    pub modules: Arc<dyn ModuleRestarter>,
    /// Archiwa wpisów.
    pub entries: Arc<dyn EntryStore>,
    /// Pobrania.
    pub downloads: Arc<dyn DownloadQueue>,
    /// Sonda.
    pub probe: Arc<dyn HealthProbe>,
}

/// Inicjator zmian konfiguracji Diagnosty (historia `core-config`).
pub fn diagnostician_origin() -> Origin {
    Origin::Module("diagnostician".into())
}

impl PortsEnv {
    async fn set_config(
        &self,
        key: &str,
        old: &Option<serde_json::Value>,
        new: &Option<serde_json::Value>,
    ) -> Result<(), String> {
        if is_kernel_key(key) || is_forbidden_key(key) {
            return Err(format!("`{key}` poza zasięgiem Diagnosty"));
        }
        let key = ConfigKey::new(key).map_err(|e| e.to_string())?;
        let current = self
            .config
            .get(&key, &Scope::Global)
            .await
            .map_err(|e| e.to_string())?;
        if current != *old {
            return Err(format!("konflikt `{key}`: wartość zmieniona od propozycji"));
        }
        self.config
            .set(
                &key,
                new.clone(),
                &Scope::Global,
                &ConfigLayer::Shared,
                diagnostician_origin(),
            )
            .await
            .map_err(|e| e.to_string())
    }
}

#[async_trait]
impl RepairEnv for PortsEnv {
    async fn apply(&self, step: &RepairStep) -> Result<RepairStep, String> {
        match step {
            RepairStep::SetConfig { key, old, new } => self.set_config(key, old, new).await?,
            RepairStep::RollbackConfig {
                from_revision,
                to_revision,
            } => {
                if self.history.current_revision().as_deref() != Some(from_revision.as_str()) {
                    return Err(format!("rewizja bieżąca ≠ {from_revision}"));
                }
                self.history.rollback_to(to_revision)?;
            }
            RepairStep::MoveFile { from, to } => self.files.move_path(from, to)?,
            RepairStep::CopyFile { from, to } => self.files.copy_file(from, to)?,
            RepairStep::RemoveCopy { path, source } => {
                self.files.remove_identical_copy(path, source)?
            }
            RepairStep::RestartModule { module } => self.modules.restart(module).await?,
            RepairStep::ArchiveEntries {
                store,
                entries,
                archive,
            } => self.entries.archive(store, *entries, archive)?,
            RepairStep::RestoreEntries {
                store,
                entries,
                archive,
            } => self.entries.restore(store, *entries, archive)?,
            RepairStep::QueueDownload { item, sha256 } => self.downloads.queue(item, sha256)?,
            RepairStep::CancelDownload { item, .. } => self.downloads.cancel(item)?,
        }
        Ok(step.clone())
    }

    async fn verify(&self, detection: &Detection) -> Result<bool, String> {
        self.probe.healthy(detection).await
    }
}
