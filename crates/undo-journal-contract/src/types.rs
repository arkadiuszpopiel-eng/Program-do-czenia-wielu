//! Typy dziennika cofania.

use std::collections::BTreeMap;
use std::path::PathBuf;

use core_bus_contract::{AgentId, RunId, SessionId};
use platform_contract::{PlatformError, UndoToken};
use serde::{Deserialize, Serialize};

/// Identyfikator kroku („Delta: przeniesiono 14 plików” = jeden krok).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StepId(pub u64);

/// Identyfikator pre-image w magazynie (SHA-256 treści, hex).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlobId(pub String);

/// Stan pliku (do wykrywania konfliktów).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FileState {
    /// Brak pliku.
    Absent,
    /// Plik o treści (SHA-256 hex) i długości.
    Present {
        /// Hash treści.
        hash: String,
        /// Długość.
        len: u64,
    },
}

/// Kontekst kroku: tura/krok agentki.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepCtx {
    /// Sesja.
    pub session: SessionId,
    /// Agentka.
    pub agent: Option<AgentId>,
    /// Przebieg planu.
    pub run: Option<RunId>,
    /// Tura rozmowy.
    pub turn: Option<String>,
    /// Etykieta kroku (np. „Porządki w Pobranych”).
    pub label: String,
    /// Czy wolno wykonać operację bez pre-image (nieodwracalną) — tylko po zgodzie Brokera.
    pub allow_irreversible: bool,
}

impl StepCtx {
    /// Kontekst minimalny.
    pub fn new(session: &str, agent: Option<&str>, label: &str) -> Self {
        Self {
            session: SessionId::new(session),
            agent: agent.map(AgentId::new),
            run: None,
            turn: None,
            label: label.to_owned(),
            allow_irreversible: false,
        }
    }
}

/// Manifest zakresu: ścieżka → (hash treści, pre-image).
pub type Manifest = BTreeMap<PathBuf, (String, BlobId)>;

/// Operacja zapisana w dzienniku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum UndoOp {
    /// Zapis (nowy plik albo nadpisanie).
    Write {
        /// Ścieżka.
        path: PathBuf,
        /// Stan przed.
        before: FileState,
        /// Pre-image (gdy plik istniał i mieścił się w limicie).
        pre_image: Option<BlobId>,
        /// Stan po.
        after: FileState,
    },
    /// Kopia.
    Copy {
        /// Źródło.
        from: PathBuf,
        /// Cel.
        to: PathBuf,
        /// Stan celu po.
        after: FileState,
    },
    /// Przeniesienie.
    Move {
        /// Źródło.
        from: PathBuf,
        /// Cel.
        to: PathBuf,
        /// Stan celu po.
        after: FileState,
    },
    /// Usunięcie do Kosza.
    Delete {
        /// Ścieżka.
        path: PathBuf,
        /// Stan przed.
        before: FileState,
        /// Pre-image (zapas po restarcie, gdy token Kosza przepadł).
        pre_image: Option<BlobId>,
    },
    /// Trwałe usunięcie (odwracalne tylko przez pre-image).
    DeletePermanent {
        /// Ścieżka.
        path: PathBuf,
        /// Stan przed.
        before: FileState,
        /// Pre-image.
        pre_image: Option<BlobId>,
    },
    /// Snapshot zakresu przed poleceniem powłoki (kopia katalogu z limitem).
    ScopeSnapshot {
        /// Korzeń zakresu.
        root: PathBuf,
        /// Stan zakresu przed.
        files: Manifest,
    },
}

/// Wpis dziennika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Krok.
    pub step: StepId,
    /// Numer w kroku.
    pub seq: u32,
    /// Operacja.
    pub op: UndoOp,
    /// Token cofnięcia platformy (ważny tylko w uruchomieniu `boot`).
    pub platform_undo: Option<UndoToken>,
    /// Uruchomienie dziennika, w którym wydano token.
    pub boot: u64,
}

/// Rekord trwałego dziennika (append-only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
pub enum JournalRecord {
    /// Początek kroku.
    Begin {
        /// Krok.
        step: StepId,
        /// Kontekst.
        ctx: StepCtx,
        /// Czas (ms).
        at_ms: u64,
    },
    /// Operacja.
    Op(JournalEntry),
    /// Zatwierdzenie kroku (stan zakresów po poleceniu powłoki: seq → ścieżka → hash).
    Commit {
        /// Krok.
        step: StepId,
        /// Czas (ms).
        at_ms: u64,
        /// Stany „po” snapshotów zakresu.
        posts: BTreeMap<u32, BTreeMap<PathBuf, String>>,
    },
    /// Krok cofnięty (`partial` = część nie wróciła — raport w zdarzeniu).
    Undone {
        /// Krok.
        step: StepId,
        /// Czy częściowo.
        partial: bool,
    },
    /// Krok przeterminowany (pre-image usunięte — nie da się cofnąć).
    Expired {
        /// Krok.
        step: StepId,
    },
}

/// Liczniki operacji kroku.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpCounts {
    /// Zapisy.
    pub written: u32,
    /// Kopie.
    pub copied: u32,
    /// Przeniesienia.
    pub moved: u32,
    /// Usunięcia do Kosza.
    pub deleted: u32,
    /// Trwałe usunięcia.
    pub purged: u32,
    /// Snapshoty zakresu.
    pub snapshots: u32,
}

/// Podsumowanie kroku (karta „Cofnij”).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSummary {
    /// Krok.
    pub step: StepId,
    /// Kontekst.
    pub ctx: StepCtx,
    /// Liczniki.
    pub counts: OpCounts,
    /// Czy cały krok da się cofnąć.
    pub reversible: bool,
    /// Zatwierdzony.
    pub committed: bool,
    /// Cofnięty.
    pub undone: bool,
    /// Przeterminowany.
    pub expired: bool,
    /// Tekst po polsku („Delta: przeniesiono 14 plików”).
    pub text: String,
}

/// Raport cofnięcia — nigdy cicho: lista tego, co nie wróciło.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoReport {
    /// Krok.
    pub step: StepId,
    /// Przywrócone operacje.
    pub restored: u32,
    /// Nieudane (ścieżka, powód).
    pub failed: Vec<(PathBuf, String)>,
}

/// Limity magazynu (`[undo]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoLimits {
    /// Maksymalny pre-image jednego pliku (domyślnie 50 MB).
    pub pre_image_max_bytes: u64,
    /// Limit magazynu (domyślnie 2 GB).
    pub store_limit_bytes: u64,
    /// Retencja kroków (domyślnie 7 dni).
    pub retention_ms: u64,
    /// Maksymalna liczba plików snapshotu zakresu.
    pub snapshot_max_files: usize,
    /// Maksymalny rozmiar snapshotu zakresu.
    pub snapshot_max_bytes: u64,
}

impl Default for UndoLimits {
    fn default() -> Self {
        Self {
            pre_image_max_bytes: 50 * 1024 * 1024,
            store_limit_bytes: 2 * 1024 * 1024 * 1024,
            retention_ms: 7 * 24 * 60 * 60 * 1000,
            snapshot_max_files: 10_000,
            snapshot_max_bytes: 500 * 1024 * 1024,
        }
    }
}

/// Błędy dziennika.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UndoError {
    /// Nieznany krok.
    #[error("nieznany krok {0:?}")]
    UnknownStep(StepId),
    /// Krok w złym stanie (zatwierdzony / niezatwierdzony / cofnięty).
    #[error("krok {step:?}: {reason}")]
    BadState {
        /// Krok.
        step: StepId,
        /// Powód.
        reason: String,
    },
    /// Krok przeterminowany — pre-image usunięte.
    #[error("krok {0:?} wygasł (retencja) — nie da się go cofnąć")]
    Expired(StepId),
    /// Plik zmieniono po operacji — cofnięcie nadpisałoby cudzą zmianę.
    #[error("konflikt: „{}” zmieniono po tej operacji — cofnięcie wstrzymane", path.display())]
    Conflict {
        /// Ścieżka.
        path: PathBuf,
        /// Stan oczekiwany.
        expected: FileState,
        /// Stan zastany.
        found: FileState,
    },
    /// Pre-image przekracza limit, a krok nie ma zgody na nieodwracalność.
    #[error("„{}” ({size} B) przekracza limit pre-image {limit} B — operacja nieodwracalna wymaga zgody", path.display())]
    PreImageTooLarge {
        /// Ścieżka.
        path: PathBuf,
        /// Rozmiar.
        size: u64,
        /// Limit.
        limit: u64,
    },
    /// Magazyn cofania pełny.
    #[error("magazyn cofania pełny")]
    StoreFull,
    /// Snapshot zakresu za duży.
    #[error("zakres „{}” za duży na snapshot ({files} plików, {bytes} B)", root.display())]
    SnapshotTooLarge {
        /// Korzeń.
        root: PathBuf,
        /// Pliki.
        files: usize,
        /// Bajty.
        bytes: u64,
    },
    /// Cofnięcie częściowe — raport wskazuje, co nie wróciło.
    #[error("cofnięcie częściowe: {} nie wróciło", .0.failed.len())]
    Partial(UndoReport),
    /// Błąd platformy (operacja nie została zapisana).
    #[error("platforma: {0}")]
    Platform(PlatformError),
    /// Błąd magazynu dziennika.
    #[error("magazyn dziennika: {0}")]
    Store(String),
}
