//! Atrapa obserwacji katalogów: wirtualny system plików (ścieżka → odcisk) z wirtualnym zegarem.
//! Operacje na plikach generują surowe zmiany jak `ReadDirectoryChangesW` (przemianowanie w obrębie
//! obserwacji = para nazw), `lose_events` udaje przepełniony bufor (zmiany przepadają), `overflow`
//! wymusza pełne przeskanowanie, `link` udaje junction (ścieżka kanoniczna sprawdzana deny-listą).
//! Rdzeń (filtr, debounce, przeskanowanie) jest wspólny z Windows: `WatchSet` z kontraktu.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use platform_contract::{
    DirWatchPort, FileStamp, PlatformError, RawChange, RescanReason, WatchEvent, WatchId,
    WatchPolicy, WatchSet, WatchSpec,
};

struct State {
    now_ms: u64,
    files: BTreeMap<PathBuf, FileStamp>,
    links: BTreeMap<PathBuf, PathBuf>,
    set: WatchSet,
    losing: bool,
    raw_seen: usize,
}

impl State {
    fn canonical(&self, path: &Path) -> PathBuf {
        for (link, target) in &self.links {
            if let Ok(rest) = path.strip_prefix(link) {
                return target.join(rest);
            }
        }
        path.to_path_buf()
    }

    fn listing(&self, spec: &WatchSpec) -> Vec<(PathBuf, FileStamp)> {
        self.files
            .iter()
            .filter(|(p, _)| spec.covers(p))
            .map(|(p, s)| (p.clone(), *s))
            .collect()
    }

    fn emit(&mut self, change: &RawChange) {
        if self.losing {
            return;
        }
        self.raw_seen += 1;
        let now = self.now_ms;
        for (id, _) in self.set.specs() {
            self.set.raw(id, now, change.clone());
        }
    }

    fn stamp(&mut self, len: u64) -> FileStamp {
        FileStamp {
            len,
            modified_ms: self.now_ms,
        }
    }

    fn rescan_all(&mut self, reason: RescanReason) {
        let now = self.now_ms;
        for (id, spec) in self.set.specs() {
            let listing = self.listing(&spec);
            self.set.rescan(id, now, reason, listing);
        }
    }
}

/// Obserwacja katalogów w pamięci.
pub struct FakeDirWatch {
    state: Mutex<State>,
}

impl std::fmt::Debug for FakeDirWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeDirWatch").finish_non_exhaustive()
    }
}

impl Default for FakeDirWatch {
    fn default() -> Self {
        Self::new(WatchPolicy::baseline())
    }
}

impl FakeDirWatch {
    /// Pusty system plików, zegar 0.
    pub fn new(policy: WatchPolicy) -> Self {
        Self {
            state: Mutex::new(State {
                now_ms: 0,
                files: BTreeMap::new(),
                links: BTreeMap::new(),
                set: WatchSet::new(policy),
                losing: false,
                raw_seen: 0,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Wirtualny czas (ms).
    pub fn now_ms(&self) -> u64 {
        self.lock().now_ms
    }

    /// Przesuwa zegar.
    pub fn advance(&self, ms: u64) {
        let mut s = self.lock();
        s.now_ms = s.now_ms.saturating_add(ms);
    }

    /// Pliki wirtualnego systemu.
    pub fn files(&self) -> Vec<PathBuf> {
        self.lock().files.keys().cloned().collect()
    }

    /// Ile surowych zmian dotarło do obserwacji (bez utraconych).
    pub fn raw_seen(&self) -> usize {
        self.lock().raw_seen
    }

    /// Plik bez zdarzeń (stan przed startem obserwacji).
    pub fn seed(&self, path: impl Into<PathBuf>, len: u64) {
        let mut s = self.lock();
        let stamp = s.stamp(len);
        s.files.insert(path.into(), stamp);
    }

    /// Junction: `link` wskazuje na `target` (ścieżka kanoniczna obserwacji).
    pub fn link(&self, link: impl Into<PathBuf>, target: impl Into<PathBuf>) {
        self.lock().links.insert(link.into(), target.into());
    }

    /// Zapis pliku (nowy → `Added`, istniejący → `Modified`).
    pub fn write(&self, path: impl Into<PathBuf>, len: u64) {
        let path = path.into();
        let mut s = self.lock();
        let stamp = s.stamp(len);
        let existed = s.files.insert(path.clone(), stamp).is_some();
        let change = if existed {
            RawChange::Modified(path, stamp)
        } else {
            RawChange::Added(path, stamp)
        };
        s.emit(&change);
    }

    /// Usunięcie pliku (brak pliku = nic).
    pub fn remove(&self, path: &Path) {
        let mut s = self.lock();
        if s.files.remove(path).is_some() {
            s.emit(&RawChange::Removed(path.to_path_buf()));
        }
    }

    /// Przemianowanie (cel istniejący jest zastępowany: `Removed` celu, potem para nazw).
    pub fn rename(&self, from: &Path, to: impl Into<PathBuf>) {
        let to = to.into();
        let mut s = self.lock();
        let Some(stamp) = s.files.remove(from) else {
            return;
        };
        if s.files.insert(to.clone(), stamp).is_some() {
            s.emit(&RawChange::Removed(to.clone()));
        }
        s.emit(&RawChange::Renamed {
            from: from.to_path_buf(),
            to,
            stamp,
        });
    }

    /// Przeniesienie katalogu: system zgłasza tylko katalog, więc obserwacje przeskanowują
    /// zawartość (`RescanReason::DirectoryMoved`).
    pub fn move_dir(&self, from: &Path, to: &Path) {
        let mut s = self.lock();
        let moved: Vec<(PathBuf, FileStamp)> = s
            .files
            .iter()
            .filter_map(|(p, st)| p.strip_prefix(from).ok().map(|r| (to.join(r), *st)))
            .collect();
        s.files.retain(|p, _| !p.starts_with(from));
        s.files.extend(moved);
        if !s.losing {
            s.rescan_all(RescanReason::DirectoryMoved);
        }
    }

    /// Przepełniony bufor: zmiany przepadają (`true`) albo znów docierają.
    pub fn lose_events(&self, losing: bool) {
        self.lock().losing = losing;
    }

    /// System zgłasza przepełnienie: każda obserwacja przeskanowuje katalog.
    pub fn overflow(&self) {
        self.lock().rescan_all(RescanReason::Overflow);
    }

    /// Katalog obserwacji zniknął (zdarzenie `Stopped`).
    pub fn stop(&self, id: WatchId, reason: &str) {
        self.lock().set.stopped(id, reason);
    }
}

impl DirWatchPort for FakeDirWatch {
    fn watch(&self, spec: WatchSpec) -> Result<WatchId, PlatformError> {
        let mut s = self.lock();
        let canonical = s.canonical(&spec.dir);
        let listing = s.listing(&spec);
        s.set.add(spec, Some(&canonical), listing)
    }

    fn unwatch(&self, id: WatchId) -> Result<(), PlatformError> {
        self.lock().set.remove(id).map(|_| ())
    }

    fn watches(&self) -> Vec<(WatchId, WatchSpec)> {
        self.lock().set.specs()
    }

    fn drain_events(&self) -> Vec<WatchEvent> {
        let mut s = self.lock();
        let now = s.now_ms;
        s.set.poll(now)
    }

    /// Wirtualny czas: przesuwa zegar do najbliższej gotowości (najwyżej o `timeout`).
    fn wait_events(&self, timeout: Duration) -> Vec<WatchEvent> {
        let mut s = self.lock();
        let now = s.now_ms;
        let ready = s.set.poll(now);
        if !ready.is_empty() {
            return ready;
        }
        let end = now.saturating_add(u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX));
        match s.set.next_due_ms(now) {
            Some(due) if due <= end => {
                s.now_ms = due.max(now);
                let at = s.now_ms;
                s.set.poll(at)
            }
            _ => {
                s.now_ms = end;
                Vec::new()
            }
        }
    }
}
