//! DTO Inspektora pamięci (F7, makieta 9; odpowiedniki `types-memory.ts`): zakresy, wpisy
//! z proweniencją, „dlaczego to pamiętam", edycja = nowa wersja, zapomnienie z podglądem kaskady,
//! dziennik zmian z cofaniem i porządkowanie (Strażniczka pamięci).
//!
//! Identyfikator wpisu w UI: `"<klucz zakresu>#<id>"` (np. `session:s-1#mem-…`, `global#mem-…`).

use serde::{Deserialize, Serialize};

use super::common::Iso8601;

/// Rodzaj zakresu pamięci.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScopeKind {
    Session,
    Project,
    Agent,
    Global,
}

/// Zakres pamięci (`id` = sesja / projekt / agentka; `null` dla globalnej).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryScopeRef {
    pub kind: MemoryScopeKind,
    pub id: Option<String>,
}

/// Warstwa pamięci.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLayer {
    Working,
    Episodic,
    Semantic,
    Procedural,
}

/// Stan wpisu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryState {
    Active,
    Pending,
    Superseded,
    Expired,
}

/// Pochodzenie wpisu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySourceKind {
    User,
    Agent,
    Untrusted,
    Import,
}

/// Zakres z danymi (lista Inspektora).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryScopeInfo {
    pub key: String,
    pub scope: MemoryScopeRef,
    pub label: String,
    pub entries: u64,
    pub active: u64,
    pub pending: u64,
    /// Nazwa dokumentu w paczce `.alfa` (`null` — sesja prywatna, poza eksportem).
    pub document: Option<String>,
}

/// Zapytanie Inspektora (filtry łączone AND; puste listy = bez filtra).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryQuery {
    pub scopes: Vec<String>,
    pub text: Option<String>,
    pub layers: Vec<MemoryLayer>,
    pub states: Vec<MemoryState>,
    pub trusted: Option<bool>,
    pub pinned: Option<bool>,
    pub offset: u64,
    pub limit: u64,
}

/// Wpis pamięci.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub scope: MemoryScopeRef,
    pub scope_key: String,
    pub layer: MemoryLayer,
    pub state: MemoryState,
    pub text: String,
    pub subject: Option<String>,
    pub entities: Vec<String>,
    pub source: MemorySourceKind,
    /// Agentka (pochodzenie `agent`) albo źródło (treść niezaufana, import).
    pub source_detail: Option<String>,
    pub trusted: bool,
    pub confidence: f64,
    pub pinned: bool,
    pub version: u32,
    pub created_at: Iso8601,
    pub expires_at: Option<Iso8601>,
    pub session_id: Option<String>,
    pub turn: Option<u64>,
    /// Jak powstał (`extracted`, `summary`, `skill`, `promoted`, `edited`, `imported`).
    pub derivation: Option<String>,
    pub score: Option<f64>,
}

/// Strona wyników.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryPage {
    pub items: Vec<MemoryItem>,
    pub total: u64,
}

/// Źródło wpisu w „dlaczego to pamiętam".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemorySourceLink {
    pub id: String,
    pub exists: bool,
    pub state: Option<MemoryState>,
}

/// Rekord dziennika zmian.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryJournalEntry {
    pub id: String,
    pub scope_key: String,
    /// Przebieg porządkowania (`null` = zmiana użytkownika).
    pub run: Option<String>,
    pub at: Iso8601,
    /// `create`, `supersede`, `merge`, `expire`, `mark_consolidated`, `conflict`, `edit`, `promote`.
    pub kind: String,
    pub note: String,
    pub entries: Vec<String>,
    pub undone: bool,
    /// Czy da się cofnąć (wygaszenia są nieodwracalne).
    pub undoable: bool,
}

/// „Dlaczego to pamiętam".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryExplanation {
    pub item: MemoryItem,
    pub reasons: Vec<String>,
    pub sources: Vec<MemorySourceLink>,
    /// Historia wersji od najstarszej (łącznie z tym wpisem).
    pub versions: Vec<MemoryItem>,
    pub merged: Vec<String>,
    pub derived: Vec<String>,
    pub journal: Vec<MemoryJournalEntry>,
}

/// Edycja = nowa wersja (pola `null` bez zmian; pusty `subject` usuwa temat).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MemoryEdit {
    pub text: Option<String>,
    pub subject: Option<String>,
    pub confidence: Option<f64>,
}

/// Co zapomnieć (kaskadowo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum MemoryForgetTarget {
    Entry { id: String },
    Scope { scope: String },
}

/// Pozycja podglądu kaskady.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryCascadeItem {
    pub id: String,
    pub scope_key: String,
    /// Początek treści (do 120 znaków).
    pub text: String,
    /// `target`, `version` albo `derived`.
    pub reason: String,
}

/// Podgląd kaskady przed zapomnieniem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryForgetPreview {
    pub target: MemoryForgetTarget,
    pub remove: Vec<MemoryCascadeItem>,
    /// Wpisy, które wrócą do stanu aktywnego (były zastąpione przez usuwane).
    pub revive: Vec<String>,
    /// Zakres zostanie usunięty w całości (crypto-shredding bazy).
    pub shred: bool,
}

/// Raport zapomnienia (weryfikowalny).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryForgetReport {
    pub removed: u64,
    pub derived: u64,
    pub versions: u64,
    pub revived: u64,
    pub fts_rows: u64,
    pub vectors: u64,
    pub journal_records: u64,
    pub shredded: Vec<String>,
    pub stale_exports: Vec<String>,
}

/// Wynik cofnięcia zmiany z dziennika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryUndoResult {
    pub removed: u64,
    pub restored: u64,
    pub skipped: u64,
}

/// Raport porządkowania pamięci (Strażniczka).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsolidationReport {
    pub run: String,
    pub manual: bool,
    pub started_at: Iso8601,
    /// Dlaczego nie wystartował (bateria, tryb gry, okno, bezczynność, wyłączona…).
    pub skipped: Option<String>,
    pub interrupted: Option<String>,
    pub scopes: u64,
    pub created: u64,
    pub merged: u64,
    pub resolved: u64,
    pub expired: u64,
    pub conflicts: u64,
    pub proposals: u64,
    pub llm_calls: u64,
    pub budget_denied: bool,
    pub errors: Vec<String>,
}

/// Stan pamięci dla Inspektora (porządkowanie, licznik bezczynności, oczekujące).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub consolidation_enabled: bool,
    /// Licznik bezczynności dostępny (bez niego harmonogram nocny nie startuje; ręcznie — tak).
    pub idle_available: bool,
    /// Model lokalny dla porządkowania (bez niego — tylko reguły deterministyczne).
    pub model_available: bool,
    pub window: String,
    pub pending: u64,
    pub last: Option<ConsolidationReport>,
}
