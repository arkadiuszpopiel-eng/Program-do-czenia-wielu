//! Pełny kontrakt pamięci F7 ([`MemoryService`]): cztery warstwy, zakresy z uprawnieniami,
//! wersjonowanie faktów, Inspektor, `forget` kaskadowo, dziennik z cofaniem, eksport/import.
//!
//! Kontrakt v0 ([`crate::Memory`]) zostaje jako fasada zakresu sesji (bez tożsamości
//! wywołującego, więc bez zakresów szerszych).

use core_bus_contract::SessionId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::access::Accessor;
use crate::error::MemoryError;
use crate::inspect::{EntryEdit, Explanation, InspectorPage, InspectorQuery};
use crate::journal::{ChangeId, ChangeReport, ChangeSet, JournalRecord, UndoReport};
use crate::model::EntryRef;
use crate::types::{Layer, MemoryEntry, MemoryScope, NewMemory, Recalled, RememberMode};

/// Żądanie `recall` F7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecallRequest {
    /// Zakresy (puste: agentka → wszystkie jej czytelne; właściciel/Strażniczka → błąd).
    #[serde(default)]
    pub scopes: Vec<MemoryScope>,
    /// Zapytanie.
    pub query: String,
    /// Liczba wyników.
    pub k: usize,
    /// Warstwy (puste = wszystkie poza roboczą, którą zwraca [`MemoryService::working_set`]).
    #[serde(default)]
    pub layers: Vec<Layer>,
}

impl RecallRequest {
    /// Żądanie w podanych zakresach.
    pub fn new(scopes: Vec<MemoryScope>, query: impl Into<String>, k: usize) -> Self {
        Self {
            scopes,
            query: query.into(),
            k,
            layers: Vec::new(),
        }
    }
}

/// Zestaw roboczy sesji (warstwa robocza: przypięte + najtrafniejsze wpisy w budżecie znaków).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorkingSet {
    /// Przypięte wpisy czytelnych zakresów (zawsze pierwsze).
    pub pinned: Vec<MemoryEntry>,
    /// Najtrafniejsze wpisy dla zapytania.
    pub recalled: Vec<Recalled>,
    /// Łączna liczba znaków treści.
    pub chars: usize,
    /// Czy budżet obciął zestaw.
    pub truncated: bool,
}

/// Co zapomnieć (kaskadowo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum ForgetTarget {
    /// Wpis z całą historią wersji i scalonymi duplikatami + pochodne.
    Entry(EntryRef),
    /// Cały zakres (baza własna → crypto-shredding) + pochodne w zakresach szerszych.
    Scope(MemoryScope),
    /// Wszystko, co pochodzi z sesji (zakres sesji i wpisy z `origin.session` w każdym zakresie).
    Session(SessionId),
    /// Wpisy wyekstrahowane z jednej tury sesji.
    Turn {
        /// Sesja.
        session: SessionId,
        /// Numer tury.
        turn: u64,
    },
    /// Treść z jednego źródła niezaufanego lub importu (URL, ścieżka) we wszystkich zakresach.
    Source(String),
}

/// Raport kaskady `forget` (weryfikowalny — PLAN §10, ACCEPTANCE F7-03).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CascadeReport {
    /// Wszystkie usunięte wpisy (nasiona, wersje, pochodne).
    pub removed: Vec<EntryRef>,
    /// Z tego: wpisy pochodne (streszczenia, fakty, kopie, nowe wersje).
    pub derived: Vec<EntryRef>,
    /// Z tego: historyczne wersje i scalone duplikaty.
    pub versions: Vec<EntryRef>,
    /// Wpisy przywrócone (były zastąpione przez wpis usunięty).
    pub revived: Vec<EntryRef>,
    /// Usunięte wiersze FTS.
    pub fts_rows: usize,
    /// Usunięte wektory.
    pub vectors: usize,
    /// Usunięte rekordy dziennika (zawierały migawki treści).
    pub journal_records: usize,
    /// Wyczyszczone wpisy pamięci podręcznej `recall`.
    pub cache_entries: usize,
    /// Zakresy usunięte w całości przez crypto-shredding (klucz + pliki).
    pub shredded: Vec<MemoryScope>,
    /// Eksporty zawierające usuniętą treść — do ponownego wygenerowania (np. `memory/global.ndjson`).
    pub stale_exports: Vec<String>,
}

/// Tryb importu zakresu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportPolicy {
    /// Dodaj brakujące i zastąp istniejące o tym samym `id`.
    #[default]
    Upsert,
    /// Zakres ma dokładnie zawartość paczki (lokalne spoza paczki → `forget` kaskadowo).
    Replace,
}

/// Wynik importu.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImportReport {
    /// Nowe wpisy.
    pub added: usize,
    /// Zastąpione wpisy.
    pub replaced: usize,
    /// Odrzucone (identyfikator, powód) — np. treść niezaufana w zakresie szerszym.
    pub rejected: Vec<(String, String)>,
    /// Usunięte lokalnie (tryb `Replace`).
    pub removed: CascadeReport,
}

/// Podsumowanie zakresu (lista Inspektora).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScopeSummary {
    /// Zakres.
    pub scope: MemoryScope,
    /// Wszystkie wpisy.
    pub entries: usize,
    /// Aktywne.
    pub active: usize,
    /// Oczekujące na zatwierdzenie.
    pub pending: usize,
}

/// Pamięć F7.
pub trait MemoryService: Send + Sync {
    /// Zapamiętuje wpis w zakresie, do którego wywołujący ma prawo zapisu. Agentka w zakresie
    /// szerszym niż sesja → wpis oczekujący (zgoda użytkownika); treść niezaufana → tylko zakres
    /// sesji i nigdy automatycznie; sesja prywatna → nigdy zakres szerszy. Temat (`subject`)
    /// zgodny z aktywnym faktem → nowa wersja (sprzeczność), bez nadpisania.
    fn remember_as(
        &self,
        who: &Accessor,
        new: NewMemory,
        mode: RememberMode,
    ) -> Result<MemoryEntry, MemoryError>;
    /// Wyszukiwanie hybrydowe w czytelnych zakresach + reranking; tylko wpisy aktywne.
    fn recall_as(
        &self,
        who: &Accessor,
        request: &RecallRequest,
    ) -> Result<Vec<Recalled>, MemoryError>;
    /// Jeden wpis (prawo odczytu zakresu).
    fn get_as(&self, who: &Accessor, entry: &EntryRef) -> Result<MemoryEntry, MemoryError>;
    /// Zestaw roboczy sesji agentki/właściciela (przypięte + recall w budżecie znaków).
    fn working_set(
        &self,
        who: &Accessor,
        session: &SessionId,
        query: Option<&str>,
        budget_chars: usize,
    ) -> Result<WorkingSet, MemoryError>;
    /// Przypina/odpina wpis (prawo zapisu zakresu).
    fn set_pinned(
        &self,
        who: &Accessor,
        entry: &EntryRef,
        pinned: bool,
    ) -> Result<MemoryEntry, MemoryError>;
    /// Zatwierdza wpis oczekujący (tylko właściciel — zgoda).
    fn approve_as(&self, who: &Accessor, entry: &EntryRef) -> Result<MemoryEntry, MemoryError>;
    /// Awans (kopia) do zakresu szerszego. Właściciel → aktywny; agentka/Strażniczka →
    /// oczekujący. Niezaufane i z sesji prywatnej → odmowa.
    fn promote_as(
        &self,
        who: &Accessor,
        entry: &EntryRef,
        to: MemoryScope,
    ) -> Result<MemoryEntry, MemoryError>;
    /// Zapomina kaskadowo (właściciel — dowolny cel; agentka — wpis w zakresie z prawem zapisu).
    fn forget_as(
        &self,
        who: &Accessor,
        target: &ForgetTarget,
    ) -> Result<CascadeReport, MemoryError>;
    /// Inspektor: lista/filtry/wyszukiwanie (właściciel, Strażniczka).
    fn inspect(&self, who: &Accessor, query: &InspectorQuery)
    -> Result<InspectorPage, MemoryError>;
    /// „Dlaczego to pamiętam” (właściciel, Strażniczka).
    fn explain(&self, who: &Accessor, entry: &EntryRef) -> Result<Explanation, MemoryError>;
    /// Edycja w Inspektorze = nowa wersja (stara zostaje w historii; właściciel).
    fn edit(
        &self,
        who: &Accessor,
        entry: &EntryRef,
        edit: &EntryEdit,
    ) -> Result<MemoryEntry, MemoryError>;
    /// Podsumowanie zakresów z danymi (właściciel, Strażniczka).
    fn scopes(&self, who: &Accessor) -> Result<Vec<ScopeSummary>, MemoryError>;
    /// Eksport zakresu (wszystkie wpisy i wersje; właściciel). Notuje eksport `name`.
    fn export_scope(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
        name: &str,
    ) -> Result<Vec<MemoryEntry>, MemoryError>;
    /// Import zakresu z paczki (właściciel; indeks budowany na nowo).
    fn import_scope(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
        entries: Vec<MemoryEntry>,
        policy: ImportPolicy,
    ) -> Result<ImportReport, MemoryError>;
    /// Zmiany konsolidacji w jednym zakresie, atomowo, z dziennikiem (Strażniczka, właściciel).
    fn apply_changes(&self, who: &Accessor, set: &ChangeSet) -> Result<ChangeReport, MemoryError>;
    /// Dziennik zakresu (właściciel, Strażniczka), malejąco po czasie.
    fn journal(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
    ) -> Result<Vec<JournalRecord>, MemoryError>;
    /// Cofa zmianę z dziennika (właściciel).
    fn undo(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
        change: &ChangeId,
    ) -> Result<UndoReport, MemoryError>;
}
