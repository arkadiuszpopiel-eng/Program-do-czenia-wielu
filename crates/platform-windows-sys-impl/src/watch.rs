//! `WinDirWatch`: `DirWatchPort` na `WatchSet` z kontraktu (polityka, debounce, przeskanowanie);
//! wątki `ReadDirectoryChangesW` (`watch_win`) karmią go zmianami surowymi. Kolejność przy
//! zakładaniu: ścieżka kanoniczna → polityka (deny-lista surowa i kanoniczna, limit) → dopiero
//! potem uchwyt katalogu i skan (katalog z deny-listy nigdy nie jest otwierany ani listowany).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use platform_contract::{
    DirWatchPort, FileStamp, PlatformError, RawChange, RescanReason, WatchEvent, WatchId,
    WatchPolicy, WatchSet, WatchSpec,
};

use crate::watcher;

/// Domyślny bufor zmian (64 KiB — górna granica dla udziałów sieciowych).
pub const DEFAULT_BUFFER_BYTES: usize = 64 * 1024;
/// Najmniejszy bufor (testy przepełnienia).
pub const MIN_BUFFER_BYTES: usize = 1024;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Konfiguracja (`[platform.watch]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirWatchConfig {
    /// Polityka (deny-lista z `compliance` dokłada `app-*`, limity, debounce).
    pub policy: WatchPolicy,
    /// Bufor `ReadDirectoryChangesW` (B; ≥ [`MIN_BUFFER_BYTES`]).
    pub buffer_bytes: usize,
}

impl Default for DirWatchConfig {
    fn default() -> Self {
        Self {
            policy: WatchPolicy::baseline(),
            buffer_bytes: DEFAULT_BUFFER_BYTES,
        }
    }
}

/// Stan współdzielony z wątkami obserwacji.
pub(crate) struct Shared {
    set: Mutex<WatchSet>,
    cv: Condvar,
    start: Instant,
}

impl Shared {
    pub(crate) fn now_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub(crate) fn set(&self) -> MutexGuard<'_, WatchSet> {
        lock(&self.set)
    }

    /// Zmiany surowe z jednego bufora (i ewentualne przeskanowanie) — budzi oczekujących.
    pub(crate) fn apply(
        &self,
        id: WatchId,
        changes: Vec<RawChange>,
        rescan: Option<(RescanReason, Vec<(PathBuf, FileStamp)>)>,
    ) {
        let now = self.now_ms();
        {
            let mut set = self.set();
            for change in changes {
                set.raw(id, now, change);
            }
            if let Some((reason, listing)) = rescan {
                set.rescan(id, now, reason, listing);
            }
        }
        self.cv.notify_all();
    }

    /// Obserwacja zakończona przez system.
    pub(crate) fn stopped(&self, id: WatchId, reason: &str) {
        self.set().stopped(id, reason);
        self.cv.notify_all();
    }

    /// Czy obserwacja zna pliki pod ścieżką.
    pub(crate) fn knows_under(&self, id: WatchId, path: &std::path::Path) -> bool {
        self.set().core(id).is_some_and(|c| c.knows_under(path))
    }
}

/// Obserwacja katalogów Windows.
pub struct WinDirWatch {
    cfg: DirWatchConfig,
    shared: Arc<Shared>,
    watchers: Mutex<BTreeMap<WatchId, watcher::Watcher>>,
}

impl std::fmt::Debug for WinDirWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WinDirWatch")
            .field("watches", &self.shared.set().specs().len())
            .finish_non_exhaustive()
    }
}

impl Default for WinDirWatch {
    fn default() -> Self {
        Self::new(DirWatchConfig::default())
    }
}

impl WinDirWatch {
    /// Pusta obserwacja z konfiguracją.
    pub fn new(cfg: DirWatchConfig) -> Self {
        let shared = Arc::new(Shared {
            set: Mutex::new(WatchSet::new(cfg.policy.clone())),
            cv: Condvar::new(),
            start: Instant::now(),
        });
        Self {
            cfg: DirWatchConfig {
                buffer_bytes: cfg.buffer_bytes.max(MIN_BUFFER_BYTES),
                ..cfg
            },
            shared,
            watchers: Mutex::new(BTreeMap::new()),
        }
    }

    /// Konfiguracja.
    pub fn config(&self) -> &DirWatchConfig {
        &self.cfg
    }
}

impl DirWatchPort for WinDirWatch {
    fn watch(&self, spec: WatchSpec) -> Result<WatchId, PlatformError> {
        let mut watchers = lock(&self.watchers);
        // Najpierw postać surowa (bez dotykania dysku), potem kanoniczna (junction, 8.3).
        self.shared.set().check(&spec, None)?;
        let canonical = std::fs::canonicalize(&spec.dir).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => PlatformError::NotFound(spec.dir.clone()),
            _ => PlatformError::Io(format!("ścieżka obserwacji: {e}")),
        })?;
        if !canonical.is_dir() {
            return Err(PlatformError::InvalidPath(spec.dir.clone()));
        }
        self.shared.set().check(&spec, Some(&canonical))?;
        let (id, w) =
            watcher::Watcher::start(&self.shared, spec, &canonical, self.cfg.buffer_bytes)?;
        watchers.insert(id, w);
        Ok(id)
    }

    fn unwatch(&self, id: WatchId) -> Result<(), PlatformError> {
        let stopped = lock(&self.watchers).remove(&id);
        let removed = self.shared.set().remove(id);
        match (stopped, removed) {
            (None, Err(e)) => Err(e),
            _ => Ok(()),
        }
    }

    fn watches(&self) -> Vec<(WatchId, WatchSpec)> {
        self.shared.set().specs()
    }

    fn drain_events(&self) -> Vec<WatchEvent> {
        let now = self.shared.now_ms();
        self.shared.set().poll(now)
    }

    fn wait_events(&self, timeout: Duration) -> Vec<WatchEvent> {
        let deadline = Instant::now() + timeout;
        let mut set = self.shared.set();
        loop {
            let now = self.shared.now_ms();
            let ready = set.poll(now);
            if !ready.is_empty() {
                return ready;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Vec::new();
            }
            let wait = set.next_due_ms(now).map_or(left, |due| {
                left.min(Duration::from_millis(due - now.min(due)))
            });
            set = self
                .shared
                .cv
                .wait_timeout(set, wait.max(Duration::from_millis(1)))
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
    }
}

impl Drop for WinDirWatch {
    fn drop(&mut self) {
        // Zatrzymanie wątków (`Watcher::drop`: zdarzenie stop, `CancelIoEx`, join).
        lock(&self.watchers).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_is_checked_before_the_platform() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".ssh")).unwrap();
        std::fs::create_dir_all(tmp.path().join("ok")).unwrap();
        let w = WinDirWatch::new(DirWatchConfig {
            buffer_bytes: 1,
            ..DirWatchConfig::default()
        });
        assert_eq!(w.config().buffer_bytes, MIN_BUFFER_BYTES);
        assert!(matches!(
            w.watch(WatchSpec::new(tmp.path().join(".ssh"))),
            Err(PlatformError::Denylisted(_))
        ));
        assert!(matches!(
            w.watch(WatchSpec::new(tmp.path().join("brak"))),
            Err(PlatformError::NotFound(_))
        ));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(tmp.path().join(".ssh"), tmp.path().join("skrot")).unwrap();
            assert!(matches!(
                w.watch(WatchSpec::new(tmp.path().join("skrot"))),
                Err(PlatformError::Denylisted(_))
            ));
        }
        if !cfg!(windows) {
            assert!(matches!(
                w.watch(WatchSpec::new(tmp.path().join("ok"))),
                Err(PlatformError::Unsupported(_))
            ));
        }
        assert!(w.unwatch(WatchId(99)).is_err());
        assert!(w.drain_events().is_empty());
        assert!(w.wait_events(Duration::from_millis(5)).is_empty());
        assert!(w.watches().is_empty());
    }
}
