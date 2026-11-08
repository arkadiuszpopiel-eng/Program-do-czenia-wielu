//! Port magazynu silnika F7 ([`crate::MemoryEngine`]): wpisy, indeks (FTS + wektor) i dziennik
//! jednego zakresu. `memory-impl` — szyfrowane bazy SQLite (sesja: baza sesji; projekt/agentka/
//! globalna: osobne bazy z kluczem w sejfie), `memory-fake` — mapy w pamięci.
//!
//! Silnik trzyma całą logikę (uprawnienia, prywatność, wersje, kaskada, konsolidacja), magazyn
//! tylko wykonuje operacje — dzięki temu atrapa i implementacja zachowują się identycznie.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::MemoryError;
use crate::journal::{ChangeId, JournalRecord};
use crate::types::{MemoryEntry, MemoryId, MemoryScope};

/// Zapytanie o kandydatów `recall` w jednym zakresie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateQuery {
    /// Tekst zapytania (do embeddingu).
    pub text: String,
    /// Rdzenie słów treściowych (do FTS z dopasowaniem „dowolne słowo”, prefiksy).
    pub stems: Vec<String>,
    /// Maksymalna liczba kandydatów.
    pub limit: usize,
}

/// Kandydat z wyszukiwania (hybryda RRF w `-impl`).
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Wpis.
    pub id: MemoryId,
    /// Wynik wyszukiwania (większy = lepszy).
    pub score: f32,
}

/// Operacja zapisu w zakresie (w jednej transakcji z indeksem).
#[derive(Debug, Clone, PartialEq)]
pub enum StoreOp {
    /// Wstawia albo zastępuje wpis (indeks aktualizowany, gdy zmieniła się treść).
    Put(Box<MemoryEntry>),
    /// Usuwa wpis z magazynu i indeksu (FTS + wektor).
    Delete(MemoryId),
    /// Wstawia albo zastępuje rekord dziennika.
    PutJournal(Box<JournalRecord>),
    /// Usuwa rekord dziennika.
    DeleteJournal(ChangeId),
    /// Notuje eksport zakresu (do raportu „eksporty do ponownego wygenerowania” po `forget`).
    NoteExport {
        /// Nazwa dokumentu eksportu.
        name: String,
        /// Kiedy.
        at: DateTime<Utc>,
    },
}

/// Wynik transakcji.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommitReport {
    /// Usunięte wpisy.
    pub entries_deleted: usize,
    /// Usunięte wiersze FTS.
    pub fts_rows: usize,
    /// Usunięte wektory.
    pub vectors: usize,
    /// Usunięte rekordy dziennika.
    pub journal_deleted: usize,
}

impl CommitReport {
    /// Suma raportów.
    pub fn add(&mut self, other: CommitReport) {
        self.entries_deleted += other.entries_deleted;
        self.fts_rows += other.fts_rows;
        self.vectors += other.vectors;
        self.journal_deleted += other.journal_deleted;
    }
}

/// Wynik usunięcia całego zakresu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DropReport {
    /// Usunięte dane (wpisy, indeks, dziennik).
    pub commit: CommitReport,
    /// Crypto-shredding: klucz bazy zakresu usunięty z sejfu i pliki skasowane.
    pub shredded: bool,
}

/// Odnotowany eksport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportNote {
    /// Nazwa dokumentu.
    pub name: String,
    /// Kiedy.
    pub at: DateTime<Utc>,
}

/// Magazyn wpisów pamięci.
pub trait MemoryBackend: Send + Sync {
    /// Wszystkie wpisy zakresu (wszystkie stany), rosnąco po `(created_at, id)`. Pusty zakres → `[]`.
    fn entries(&self, scope: &MemoryScope) -> Result<Vec<MemoryEntry>, MemoryError>;
    /// Jeden wpis.
    fn entry(&self, scope: &MemoryScope, id: &MemoryId)
    -> Result<Option<MemoryEntry>, MemoryError>;
    /// Kandydaci (malejąco po wyniku, deterministycznie).
    fn search(
        &self,
        scope: &MemoryScope,
        query: &CandidateQuery,
    ) -> Result<Vec<Candidate>, MemoryError>;
    /// Atomowo stosuje operacje w zakresie. Po usunięciach magazyn zaciera dane (secure delete,
    /// kompakcja indeksu FTS, checkpoint WAL).
    fn commit(&self, scope: &MemoryScope, ops: Vec<StoreOp>) -> Result<CommitReport, MemoryError>;
    /// Dziennik zakresu, malejąco po czasie.
    fn journal(&self, scope: &MemoryScope) -> Result<Vec<JournalRecord>, MemoryError>;
    /// Odnotowane eksporty zakresu.
    fn exports(&self, scope: &MemoryScope) -> Result<Vec<ExportNote>, MemoryError>;
    /// Usuwa cały zakres: baza własna (projekt/agentka/globalna) → crypto-shredding (klucz +
    /// pliki); zakres sesji → wszystkie wiersze pamięci w bazie sesji (bazę i klucz sesji usuwa
    /// moduł `sessions`).
    fn drop_scope(&self, scope: &MemoryScope) -> Result<DropReport, MemoryError>;
    /// Zakresy z danymi (deterministyczny porządek).
    fn scopes(&self) -> Result<Vec<MemoryScope>, MemoryError>;
}
