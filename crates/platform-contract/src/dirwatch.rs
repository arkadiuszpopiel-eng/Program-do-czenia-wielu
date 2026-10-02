//! Obserwacja katalogów (wyzwalacze „nowy plik w katalogu”, PLAN §9.5; Windows:
//! `ReadDirectoryChangesW`): specyfikacja obserwacji, polityka (deny-lista poświadczeń — katalogi
//! z deny-listy nigdy nie są obserwowane, a zmiany pod nimi nigdy nie wychodzą; limit liczby
//! obserwacji), filtr nazw (wzorce, pliki tymczasowe) i port. Debounce i pełne przeskanowanie po
//! przepełnieniu bufora: `dirwatch_core` (wspólne dla Windows i atrapy).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::dirwatch_policy::{glob_match, is_temp_name};
use crate::error::PlatformError;

/// Domyślny limit obserwowanych katalogów.
pub const DEFAULT_MAX_WATCHES: usize = 16;
/// Domyślny limit plików pamiętanych na obserwację (pełne przeskanowanie po przepełnieniu).
pub const DEFAULT_MAX_WATCH_ENTRIES: usize = 4_096;
/// Domyślna cisza, po której zmiana pliku jest zgłaszana (ms).
pub const DEFAULT_DEBOUNCE_MS: u64 = 750;
/// Najdłuższe opóźnienie zgłoszenia pliku zmienianego bez przerwy (ms).
pub const DEFAULT_MAX_DELAY_MS: u64 = 30_000;
/// Najwięcej wzorców nazw w obserwacji.
pub const MAX_WATCH_PATTERNS: usize = 16;
/// Najdłuższy wzorzec nazwy.
pub const MAX_PATTERN_LEN: usize = 128;

/// Segmenty ścieżek nigdy nieobserwowane (bazowa deny-lista Jądra z `compliance` + znaczniki
/// poświadczeń kontraktu platformy). `app-*` dokłada aktualną listę przez `WatchPolicy::with_denylist`.
pub const BASELINE_DENY_SEGMENTS: [&str; 17] = [
    ".claude",
    ".claude.json",
    ".codex",
    ".gemini",
    ".grok",
    ".kimi",
    ".agy",
    ".ssh",
    ".gnupg",
    ".aws",
    ".azure",
    ".kube",
    ".npmrc",
    ".pypirc",
    ".netrc",
    "_netrc",
    ".git-credentials",
];

/// Identyfikator obserwacji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WatchId(pub u64);

/// Co obserwować.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchSpec {
    /// Katalog (ścieżka bezwzględna).
    pub dir: PathBuf,
    /// Także podkatalogi.
    #[serde(default)]
    pub recursive: bool,
    /// Wzorce nazw plików (`*`, `?`, bez rozróżniania wielkości liter); puste = wszystkie.
    #[serde(default)]
    pub patterns: Vec<String>,
    /// Pomijaj pliki tymczasowe (pobieranie w toku, blokady edytorów).
    #[serde(default = "default_true")]
    pub ignore_temp: bool,
}

fn default_true() -> bool {
    true
}

impl WatchSpec {
    /// Obserwacja katalogu (bez podkatalogów, wszystkie pliki, bez tymczasowych).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            recursive: false,
            patterns: Vec::new(),
            ignore_temp: true,
        }
    }

    /// Z podkatalogami.
    #[must_use]
    pub fn recursive(mut self) -> Self {
        self.recursive = true;
        self
    }

    /// Wzorce nazw.
    #[must_use]
    pub fn with_patterns<I: IntoIterator<Item = S>, S: Into<String>>(mut self, p: I) -> Self {
        self.patterns = p.into_iter().map(Into::into).collect();
        self
    }

    /// Czy nazwa pliku przechodzi filtr (wzorce, pliki tymczasowe).
    pub fn accepts_name(&self, name: &str) -> bool {
        if self.ignore_temp && is_temp_name(name) {
            return false;
        }
        self.patterns.is_empty() || self.patterns.iter().any(|p| glob_match(p, name))
    }

    /// Czy ścieżka leży w zakresie obserwacji (katalog, ewentualnie podkatalogi).
    pub fn covers(&self, path: &Path) -> bool {
        let Ok(rel) = path.strip_prefix(&self.dir) else {
            return false;
        };
        let depth = rel.components().count();
        depth >= 1 && (self.recursive || depth == 1)
    }
}

/// Rodzaj zmiany.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum FsChangeKind {
    /// Nowy plik.
    Created,
    /// Zmieniona treść.
    Modified,
    /// Usunięty.
    Removed,
    /// Przemianowany (albo przeniesiony w obrębie obserwacji).
    Renamed {
        /// Poprzednia ścieżka.
        from: PathBuf,
    },
}

/// Powód pełnego przeskanowania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RescanReason {
    /// Przepełnienie bufora zmian (część zdarzeń utracona).
    Overflow,
    /// Przeniesiono lub usunięto podkatalog (zmiany plików wewnątrz nie mają zdarzeń).
    DirectoryMoved,
}

/// Zdarzenie obserwacji (ładunek `platform.fs.*`; ścieżki bez treści plików).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WatchEvent {
    /// Zmiana pliku (po debounce).
    Changed {
        /// Obserwacja.
        watch: WatchId,
        /// Plik (ścieżka bezwzględna).
        path: PathBuf,
        /// Rodzaj.
        change: FsChangeKind,
    },
    /// Pełne przeskanowanie; różnice weszły jako zwykłe zmiany.
    Rescanned {
        /// Obserwacja.
        watch: WatchId,
        /// Powód.
        reason: RescanReason,
        /// Ile różnic wykryto.
        changes: usize,
        /// Limit plików przekroczony — stan częściowy.
        truncated: bool,
    },
    /// Obserwacja zakończona (katalog usunięty, brak dostępu).
    Stopped {
        /// Obserwacja.
        watch: WatchId,
        /// Powód (bez treści).
        reason: String,
    },
}

/// Zdarzenie: zmiana pliku.
pub const EVENT_FS_CHANGED: &str = "platform.fs.changed";
/// Zdarzenie: pełne przeskanowanie.
pub const EVENT_FS_RESCANNED: &str = "platform.fs.rescanned";
/// Zdarzenie: obserwacja zakończona.
pub const EVENT_FS_WATCH_STOPPED: &str = "platform.fs.watch_stopped";

impl WatchEvent {
    /// Nazwa zdarzenia na magistrali.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Changed { .. } => EVENT_FS_CHANGED,
            Self::Rescanned { .. } => EVENT_FS_RESCANNED,
            Self::Stopped { .. } => EVENT_FS_WATCH_STOPPED,
        }
    }

    /// Nowy plik dla wyzwalaczy (`TriggersModule::file_created`): utworzony albo przemianowany
    /// z nazwy nieobserwowanej / tymczasowej (pobieranie zakończone).
    pub fn new_file(&self) -> Option<&Path> {
        match self {
            Self::Changed {
                path,
                change: FsChangeKind::Created | FsChangeKind::Renamed { .. },
                ..
            } => Some(path),
            _ => None,
        }
    }
}

/// Port obserwacji katalogów. Zdarzenia po debounce; po przepełnieniu bufora — pełne
/// przeskanowanie (`Rescanned` + różnice jako zwykłe zmiany).
pub trait DirWatchPort: Send + Sync {
    /// Zaczyna obserwację (polityka: deny-lista, limit, wzorce).
    fn watch(&self, spec: WatchSpec) -> Result<WatchId, PlatformError>;
    /// Kończy obserwację (oczekujące zmiany przepadają).
    fn unwatch(&self, id: WatchId) -> Result<(), PlatformError>;
    /// Aktywne obserwacje.
    fn watches(&self) -> Vec<(WatchId, WatchSpec)>;
    /// Zdarzenia gotowe teraz.
    fn drain_events(&self) -> Vec<WatchEvent>;
    /// Jak `drain_events`, ale czeka na zdarzenie co najwyżej `timeout`.
    fn wait_events(&self, timeout: Duration) -> Vec<WatchEvent>;

    /// Zastępuje zbiór obserwacji (adapter `FileWatchPort` wyzwalaczy): identyczne zostają (bez
    /// utraty zdarzeń), zbędne kończy, nowe zaczyna. Wynik w kolejności `specs`.
    fn replace_all(&self, specs: Vec<WatchSpec>) -> Vec<Result<WatchId, PlatformError>> {
        let current = self.watches();
        for (id, spec) in &current {
            if !specs.contains(spec) {
                let _ = self.unwatch(*id);
            }
        }
        specs
            .into_iter()
            .map(|spec| match current.iter().find(|(_, s)| *s == spec) {
                Some((id, _)) => Ok(*id),
                None => self.watch(spec),
            })
            .collect()
    }
}
