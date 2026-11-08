//! Rdzeń obserwacji (czysty, wspólny dla `ReadDirectoryChangesW` i atrapy): filtr ścieżek
//! (zakres, deny-lista, nazwy), debounce z semantyką istnienia (utworzony+usunięty w oknie = nic,
//! usunięty+utworzony = zmieniony, tymczasowy→docelowy = utworzony), pary przemianowań, stan
//! znanych plików i pełne przeskanowanie po przepełnieniu bufora (różnica stanu → zmiany).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::dirwatch::{FsChangeKind, RescanReason, WatchEvent, WatchId, WatchSpec};
use crate::dirwatch_policy::WatchPolicy;

/// Odcisk pliku (rozmiar, czas modyfikacji) — wykrywa zmiany przy przeskanowaniu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileStamp {
    /// Rozmiar (B).
    pub len: u64,
    /// Czas modyfikacji (ms od epoki).
    pub modified_ms: u64,
}

/// Surowa zmiana od systemu (przed filtrem i debounce).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawChange {
    /// Dodano plik.
    Added(PathBuf, FileStamp),
    /// Zmieniono plik.
    Modified(PathBuf, FileStamp),
    /// Usunięto plik.
    Removed(PathBuf),
    /// Przemianowano.
    Renamed {
        /// Stara ścieżka.
        from: PathBuf,
        /// Nowa ścieżka.
        to: PathBuf,
        /// Odcisk pliku pod nową nazwą.
        stamp: FileStamp,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    before: bool,
    now: bool,
    changed: bool,
    renamed_from: Option<PathBuf>,
    moved_to: Option<PathBuf>,
    first_ms: u64,
    last_ms: u64,
}

/// Stan jednej obserwacji.
#[derive(Debug, Clone)]
pub struct WatchCore {
    id: WatchId,
    spec: WatchSpec,
    policy: WatchPolicy,
    known: BTreeMap<PathBuf, FileStamp>,
    truncated: bool,
    pending: BTreeMap<PathBuf, Pending>,
}

impl WatchCore {
    /// Nowa obserwacja ze stanem początkowym `listing` (pliki w zakresie; nadmiar ponad limit
    /// i ścieżki spoza filtra są pomijane).
    pub fn new(
        id: WatchId,
        spec: WatchSpec,
        policy: WatchPolicy,
        listing: Vec<(PathBuf, FileStamp)>,
    ) -> Self {
        let mut core = Self {
            id,
            spec,
            policy,
            known: BTreeMap::new(),
            truncated: false,
            pending: BTreeMap::new(),
        };
        let (known, truncated) = core.admit(listing);
        core.known = known;
        core.truncated = truncated;
        core
    }

    /// Specyfikacja.
    pub fn spec(&self) -> &WatchSpec {
        &self.spec
    }

    /// Znane pliki (ostatni stan wg zmian surowych).
    pub fn known(&self) -> impl Iterator<Item = &PathBuf> {
        self.known.keys()
    }

    /// Czy znane są pliki pod `path` (to był katalog — jego przeniesienie wymaga przeskanowania).
    pub fn knows_under(&self, path: &Path) -> bool {
        self.known
            .range(path.to_path_buf()..)
            .take_while(|(k, _)| k.starts_with(path))
            .any(|(k, _)| k.as_path() != path)
    }

    /// Czy ścieżka uczestniczy w obserwacji: w zakresie, poza deny-listą, nazwa przechodzi filtr.
    pub fn relevant(&self, path: &Path) -> bool {
        self.spec.covers(path)
            && !self.policy.is_denied(path)
            && path
                .file_name()
                .is_some_and(|n| self.spec.accepts_name(&n.to_string_lossy()))
    }

    fn admit(&self, listing: Vec<(PathBuf, FileStamp)>) -> (BTreeMap<PathBuf, FileStamp>, bool) {
        let mut out = BTreeMap::new();
        let mut truncated = false;
        for (path, stamp) in listing.into_iter().filter(|(p, _)| self.relevant(p)) {
            if out.len() >= self.policy.max_entries {
                truncated = true;
                break;
            }
            out.insert(path, stamp);
        }
        (out, truncated)
    }

    fn touch(&mut self, path: &Path, now_ms: u64, existed: bool) -> &mut Pending {
        let e = self.pending.entry(path.to_path_buf()).or_insert(Pending {
            before: existed,
            now: existed,
            changed: false,
            renamed_from: None,
            moved_to: None,
            first_ms: now_ms,
            last_ms: now_ms,
        });
        e.last_ms = now_ms;
        e.changed = true;
        e
    }

    fn remember(&mut self, path: PathBuf, stamp: FileStamp) {
        if self.known.contains_key(&path) || self.known.len() < self.policy.max_entries {
            self.known.insert(path, stamp);
        } else {
            self.truncated = true;
        }
    }

    /// Czy plik istniał wg znanego stanu (przy stanie częściowym — zakładamy, że tak).
    fn existed(&self, path: &Path) -> bool {
        self.truncated || self.known.contains_key(path)
    }

    /// Surowa zmiana w chwili `now_ms`.
    pub fn raw(&mut self, now_ms: u64, change: RawChange) {
        match change {
            RawChange::Added(p, s) | RawChange::Modified(p, s) if self.relevant(&p) => {
                let existed = self.known.contains_key(&p);
                self.touch(&p, now_ms, existed).now = true;
                self.remember(p, s);
            }
            RawChange::Removed(p) if self.relevant(&p) => {
                let existed = self.existed(&p);
                self.touch(&p, now_ms, existed).now = false;
                self.known.remove(&p);
            }
            RawChange::Renamed { from, to, stamp } => {
                match (self.relevant(&from), self.relevant(&to)) {
                    (true, true) => self.rename(now_ms, from, to, stamp),
                    (true, false) => self.raw(now_ms, RawChange::Removed(from)),
                    (false, true) => self.raw(now_ms, RawChange::Added(to, stamp)),
                    (false, false) => {}
                }
            }
            _ => {}
        }
    }

    fn rename(&mut self, now_ms: u64, from: PathBuf, to: PathBuf, stamp: FileStamp) {
        let fresh = !self.pending.contains_key(&from) && !self.pending.contains_key(&to);
        let to_existed = self.known.contains_key(&to);
        let from_existed = self.existed(&from);
        let e_from = self.touch(&from, now_ms, from_existed);
        e_from.now = false;
        if fresh {
            e_from.moved_to = Some(to.clone());
        }
        let e_to = self.touch(&to, now_ms, to_existed);
        e_to.now = true;
        if fresh && !to_existed {
            e_to.renamed_from = Some(from.clone());
        }
        self.known.remove(&from);
        self.remember(to, stamp);
    }

    /// Przepełnienie bufora albo zmiana podkatalogu: `listing` = bieżąca zawartość (pliki).
    /// Różnice względem znanego stanu wchodzą jako zmiany surowe.
    pub fn rescan(
        &mut self,
        now_ms: u64,
        reason: RescanReason,
        listing: Vec<(PathBuf, FileStamp)>,
    ) -> WatchEvent {
        let (fresh, truncated) = self.admit(listing);
        let mut diff = Vec::new();
        for path in self.known.keys().filter(|p| !fresh.contains_key(*p)) {
            diff.push(RawChange::Removed(path.clone()));
        }
        for (path, stamp) in &fresh {
            match self.known.get(path) {
                None => diff.push(RawChange::Added(path.clone(), *stamp)),
                Some(old) if old != stamp => diff.push(RawChange::Modified(path.clone(), *stamp)),
                Some(_) => {}
            }
        }
        let changes = diff.len();
        self.truncated = false;
        for change in diff {
            self.raw(now_ms, change);
        }
        self.known = fresh;
        self.truncated = truncated;
        WatchEvent::Rescanned {
            watch: self.id,
            reason,
            changes,
            truncated,
        }
    }

    fn due(&self, e: &Pending, now_ms: u64) -> bool {
        now_ms >= e.last_ms.saturating_add(self.policy.debounce_ms)
            || now_ms >= e.first_ms.saturating_add(self.policy.max_delay_ms)
    }

    /// Źródło przemianowania czeka na swój cel (zgłaszane razem jako `Renamed`).
    fn held(&self, path: &Path, e: &Pending) -> bool {
        e.before
            && !e.now
            && e.moved_to.as_ref().is_some_and(|t| {
                self.pending
                    .get(t)
                    .is_some_and(|et| et.renamed_from.as_deref() == Some(path))
            })
    }

    fn event(&self, path: PathBuf, change: FsChangeKind) -> WatchEvent {
        WatchEvent::Changed {
            watch: self.id,
            path,
            change,
        }
    }

    /// Zmiany gotowe w chwili `now_ms` (wszystkie przy `flush`), w kolejności pierwszej zmiany.
    /// Po wywołaniu nic gotowego nie zostaje (źródło przemianowania zwolnione przez cel w tej
    /// samej rundzie wychodzi w następnej iteracji pętli).
    pub fn poll(&mut self, now_ms: u64, flush: bool) -> Vec<WatchEvent> {
        let mut out = Vec::new();
        loop {
            let mut ready: Vec<(u64, PathBuf)> = self
                .pending
                .iter()
                .filter(|(p, e)| (flush || self.due(e, now_ms)) && !self.held(p, e))
                .map(|(p, e)| (e.first_ms, p.clone()))
                .collect();
            if ready.is_empty() {
                return out;
            }
            ready.sort();
            for (_, path) in ready {
                if let Some(ev) = self.emit(path) {
                    out.push(ev);
                }
            }
        }
    }

    fn emit(&mut self, path: PathBuf) -> Option<WatchEvent> {
        let e = self.pending.remove(&path)?;
        if let Some(from) = e.renamed_from.clone().filter(|_| e.now && !e.before)
            && self.pending.get(&from).is_some_and(|ef| {
                ef.before && !ef.now && ef.moved_to.as_deref() == Some(path.as_path())
            })
        {
            self.pending.remove(&from);
            return Some(self.event(path, FsChangeKind::Renamed { from }));
        }
        let change = match (e.before, e.now) {
            (false, true) => FsChangeKind::Created,
            (true, false) => FsChangeKind::Removed,
            (true, true) if e.changed => FsChangeKind::Modified,
            _ => return None,
        };
        Some(self.event(path, change))
    }

    /// Najbliższa chwila, w której coś będzie gotowe.
    pub fn next_due_ms(&self) -> Option<u64> {
        self.pending
            .values()
            .map(|e| {
                e.last_ms
                    .saturating_add(self.policy.debounce_ms)
                    .min(e.first_ms.saturating_add(self.policy.max_delay_ms))
            })
            .min()
    }
}
