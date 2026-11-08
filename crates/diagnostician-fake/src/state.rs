//! Stan świata chaosowego i semantyka kroków naprawy na nim (porównaj-i-zamień, pliki bez
//! nadpisywania, archiwa wpisów, kolejka pobrań).

use std::collections::{BTreeMap, BTreeSet};

use diagnostician_contract::RepairStep;
use serde_json::Value;

/// Plik w świecie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    /// Rozmiar (MB).
    pub size_mb: u64,
    /// Czy treść jest poprawna (hash, walidacja).
    pub valid: bool,
}

/// Stan świata (porównywany przed/po — cofalność).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorldState {
    /// Konfiguracja wynikowa.
    pub config: BTreeMap<String, Value>,
    /// Bieżąca rewizja konfiguracji.
    pub revision: String,
    /// Rewizje (migawki konfiguracji).
    pub revisions: BTreeMap<String, BTreeMap<String, Value>>,
    /// Ostatnia dobra rewizja.
    pub last_good: Option<String>,
    /// Pliki (ścieżka → informacja); wolumin z prefiksu `D:/`, reszta na `C:`.
    pub files: BTreeMap<String, FileInfo>,
    /// Pojemność woluminów (MB).
    pub volumes: BTreeMap<String, u64>,
    /// Magazyny wpisów (dziennik cofania, segmenty logów) → liczba wpisów.
    pub entries: BTreeMap<String, u64>,
    /// Archiwa wpisów.
    pub archives: BTreeMap<String, u64>,
    /// Kolejka pobrań.
    pub downloads: BTreeSet<String>,
    /// Katalogi bez prawa zapisu.
    pub read_only_dirs: BTreeSet<String>,
    /// Porty zajęte przez obce procesy.
    pub ports_taken: BTreeSet<u16>,
    /// Przesunięcie zegara systemowego (ms).
    pub clock_skew_ms: i64,
    /// Sieć dostępna.
    pub network_up: bool,
    /// GPU działa.
    pub gpu_ok: bool,
    /// Sidecary z wadą (wysypują się poza trybem zachowawczym).
    pub sidecar_faults: BTreeSet<String>,
    /// Trasy z unieważnionym kluczem.
    pub revoked_routes: BTreeSet<String>,
    /// Trasy z wyczerpanym limitem dostawcy.
    pub throttled_routes: BTreeSet<String>,
    /// Budżet wyczerpany.
    pub budget_exhausted: bool,
}

/// Pojemność magazynów wpisów.
pub const ENTRY_CAPACITY: [(&str, u64); 2] = [("undo-journal", 10_000), ("logs/diagnostics", 500)];
/// Minimalne wolne miejsce (MB).
pub const MIN_FREE_MB: u64 = 500;

/// Wolumin ścieżki.
pub fn volume_of(path: &str) -> &'static str {
    if path.starts_with("D:/") { "D:" } else { "C:" }
}

impl WorldState {
    /// Wolne miejsce na woluminie (MB).
    pub fn free_mb(&self, volume: &str) -> u64 {
        let used: u64 = self
            .files
            .iter()
            .filter(|(p, _)| volume_of(p) == volume)
            .map(|(_, f)| f.size_mb)
            .sum();
        self.volumes
            .get(volume)
            .copied()
            .unwrap_or(0)
            .saturating_sub(used)
    }

    /// Pojemność magazynu.
    pub fn capacity(store: &str) -> u64 {
        ENTRY_CAPACITY
            .iter()
            .find(|(s, _)| *s == store)
            .map_or(u64::MAX, |(_, c)| *c)
    }

    pub(crate) fn apply(&mut self, step: &RepairStep) -> Result<RepairStep, String> {
        match step {
            RepairStep::SetConfig { key, old, new } => {
                if self.config.get(key) != old.as_ref() {
                    return Err(format!("konflikt `{key}`"));
                }
                match new {
                    Some(v) => self.config.insert(key.clone(), v.clone()),
                    None => self.config.remove(key),
                };
            }
            RepairStep::RollbackConfig {
                from_revision,
                to_revision,
            } => {
                if self.revision != *from_revision {
                    return Err(format!(
                        "rewizja bieżąca {} ≠ {from_revision}",
                        self.revision
                    ));
                }
                // Stan bieżący JEST rewizją `from` — cofnięcie (rollback do `from`) przywróci go 1:1.
                self.revisions
                    .insert(from_revision.clone(), self.config.clone());
                let target = self
                    .revisions
                    .get(to_revision)
                    .cloned()
                    .ok_or("brak rewizji")?;
                self.config = target;
                self.revision.clone_from(to_revision);
            }
            RepairStep::MoveFile { from, to } => {
                if self.files.contains_key(to) {
                    return Err(format!("{to} już istnieje"));
                }
                let f = self
                    .files
                    .remove(from)
                    .ok_or_else(|| format!("brak {from}"))?;
                self.files.insert(to.clone(), f);
            }
            RepairStep::RemoveCopy { path, source } => {
                let same =
                    self.files.contains_key(path) && self.files.get(path) == self.files.get(source);
                if !same {
                    return Err(format!("{path} nie jest identyczną kopią {source}"));
                }
                self.files.remove(path);
            }
            RepairStep::CopyFile { from, to } => {
                if self.files.contains_key(to) {
                    return Err(format!("{to} już istnieje"));
                }
                let f = self
                    .files
                    .get(from)
                    .cloned()
                    .ok_or_else(|| format!("brak {from}"))?;
                self.files.insert(to.clone(), f);
            }
            RepairStep::RestartModule { .. } => {}
            RepairStep::ArchiveEntries {
                store,
                entries,
                archive,
            } => {
                let have = self.entries.get(store).copied().unwrap_or(0);
                if have < *entries {
                    return Err(format!("{store}: za mało wpisów"));
                }
                self.entries.insert(store.clone(), have - entries);
                *self.archives.entry(archive.clone()).or_default() += entries;
            }
            RepairStep::RestoreEntries {
                store,
                entries,
                archive,
            } => {
                let have = self.archives.get(archive).copied().unwrap_or(0);
                if have < *entries {
                    return Err(format!("{archive}: za mało wpisów"));
                }
                if have == *entries {
                    self.archives.remove(archive);
                } else {
                    self.archives.insert(archive.clone(), have - entries);
                }
                *self.entries.entry(store.clone()).or_default() += entries;
            }
            RepairStep::QueueDownload { item, .. } => {
                if !self.downloads.insert(item.clone()) {
                    return Err(format!("{item} już w kolejce"));
                }
            }
            RepairStep::CancelDownload { item, .. } => {
                if !self.downloads.remove(item) {
                    return Err(format!("{item} nie było w kolejce"));
                }
            }
        }
        Ok(step.clone())
    }
}
