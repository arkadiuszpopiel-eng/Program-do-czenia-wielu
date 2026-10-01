//! Silnik pamięci F7 — cała logika (uprawnienia, prywatność, wersje, kaskada, dziennik, recall
//! z rerankingiem, Inspektor) nad portem magazynu [`MemoryBackend`]. `memory-impl` i
//! `memory-fake` dostarczają tylko magazyn, więc atrapa zachowuje się jak implementacja.

mod cache;
mod changes;
mod explain;
mod forget;
mod read;
mod transfer;
mod undo;
mod v0;
mod write;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use core_bus_contract::SessionId;
use lib_sqlstore::search_tokens;

use crate::access::Accessor;
use crate::backend::{CommitReport, MemoryBackend, StoreOp};
use crate::error::MemoryError;
use crate::inspect::{EntryEdit, Explanation, InspectorPage, InspectorQuery};
use crate::journal::{ChangeId, ChangeKind, ChangeReport, ChangeSet, JournalRecord, UndoReport};
use crate::model::{EntryRef, is_broader, validate_scope};
use crate::ports::{
    EventSink, IdSource, MemoryClock, NoEvents, PrivacyOracle, PrivateSessions, SeqIds,
    SystemClock, VirtualClock,
};
use crate::rerank::{HeuristicReranker, Reranker};
use crate::service::{
    CascadeReport, ForgetTarget, ImportPolicy, ImportReport, MemoryService, RecallRequest,
    ScopeSummary, WorkingSet,
};
use crate::types::{Layer, MemoryEntry, MemoryScope, NewMemory, Recalled, RememberMode};

pub use cache::{CACHE_CAPACITY, CACHE_TTL_SECS};
pub use read::candidate_limit;
pub use transfer::{MAX_ID_LEN, check_imported};
pub use write::trust_rank;

/// Maksymalna długość treści wpisu (znaki).
pub const MAX_TEXT_CHARS: usize = 32_768;
/// Maksymalna liczba encji wpisu.
pub const MAX_ENTITIES: usize = 32;
/// Maksymalna długość tematu/encji (znaki).
pub const MAX_LABEL_CHARS: usize = 128;

/// Porty silnika.
#[derive(Clone)]
pub struct EnginePorts {
    /// Zegar.
    pub clock: Arc<dyn MemoryClock>,
    /// Identyfikatory wpisów i zmian.
    pub ids: Arc<dyn IdSource>,
    /// Prywatność sesji.
    pub privacy: Arc<dyn PrivacyOracle>,
    /// Reranking `recall`.
    pub reranker: Arc<dyn Reranker>,
    /// Zdarzenia `memory.*`.
    pub events: Arc<dyn EventSink>,
}

impl EnginePorts {
    /// Porty produkcyjne: zegar systemowy, reranker heurystyczny, bez zdarzeń (ustawia moduł).
    pub fn system(ids: Arc<dyn IdSource>, privacy: Arc<dyn PrivacyOracle>) -> Self {
        Self {
            clock: Arc::new(SystemClock),
            ids,
            privacy,
            reranker: Arc::new(HeuristicReranker),
            events: Arc::new(NoEvents),
        }
    }

    /// Porty deterministyczne (atrapa, testy): zegar wirtualny, kolejne identyfikatory, wszystkie
    /// sesje publiczne.
    pub fn deterministic() -> Self {
        Self {
            clock: Arc::new(VirtualClock::new()),
            ids: Arc::new(SeqIds::new()),
            privacy: Arc::new(PrivateSessions::new()),
            reranker: Arc::new(HeuristicReranker),
            events: Arc::new(NoEvents),
        }
    }
}

/// Silnik pamięci F7 nad magazynem `B`.
pub struct MemoryEngine<B> {
    backend: B,
    ports: EnginePorts,
    cache: cache::RecallCache,
}

impl<B: MemoryBackend> MemoryEngine<B> {
    /// Nowy silnik.
    pub fn new(backend: B, ports: EnginePorts) -> Self {
        Self {
            backend,
            ports,
            cache: cache::RecallCache::default(),
        }
    }

    /// Magazyn (testy, diagnostyka).
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Porty.
    pub fn ports(&self) -> &EnginePorts {
        &self.ports
    }

    fn now(&self) -> DateTime<Utc> {
        self.ports.clock.now()
    }

    fn new_id(&self, prefix: &str) -> String {
        self.ports.ids.next_id(prefix)
    }

    fn load(&self, r: &EntryRef) -> Result<MemoryEntry, MemoryError> {
        self.backend
            .entry(&r.scope, &r.id)?
            .ok_or_else(|| MemoryError::NotFound { id: r.id.clone() })
    }

    /// Zapis w zakresie; każda zmiana unieważnia pamięć podręczną `recall`.
    fn commit(&self, scope: &MemoryScope, ops: Vec<StoreOp>) -> Result<CommitReport, MemoryError> {
        if ops.is_empty() {
            return Ok(CommitReport::default());
        }
        self.cache.clear();
        self.backend.commit(scope, ops)
    }

    /// Pusty rekord dziennika (wywołujący uzupełnia `refs`, `before`, `after`, `run`).
    pub(super) fn journal_record(
        &self,
        scope: &MemoryScope,
        kind: ChangeKind,
        note: impl Into<String>,
    ) -> JournalRecord {
        JournalRecord {
            id: ChangeId(self.new_id("chg")),
            run: None,
            at: self.now(),
            scope: scope.clone(),
            kind,
            refs: Vec::new(),
            before: Vec::new(),
            after: Vec::new(),
            note: note.into(),
            undone: false,
        }
    }

    /// Reguły proweniencji i prywatności wspólne dla zapisu i zmian konsolidacji.
    pub(super) fn check_flow(&self, new: &NewMemory) -> Result<(), MemoryError> {
        let broader = !matches!(new.scope, MemoryScope::Session(_));
        if broader && !new.provenance.is_trusted() {
            return Err(MemoryError::UntrustedCannotPromote);
        }
        if broader
            && let Some(s) = &new.origin.session
            && self.ports.privacy.is_private(s)
        {
            return Err(MemoryError::PrivateSource {
                session: s.to_string(),
            });
        }
        Ok(())
    }

    fn emit(&self, kind: &str, session: Option<&SessionId>, payload: serde_json::Value) {
        self.ports.events.emit(kind, session, payload);
    }

    /// Zakresy szersze niż sesja, które mają dane (miejsca możliwych pochodnych).
    fn broader_scopes(&self) -> Result<Vec<MemoryScope>, MemoryError> {
        Ok(self
            .backend
            .scopes()?
            .into_iter()
            .filter(|s| !matches!(s, MemoryScope::Session(_)))
            .collect())
    }
}

/// Klucz tematu (porównanie sprzeczności): słowa złożone `fold_pl`, małe litery.
pub fn subject_key(subject: &str) -> String {
    search_tokens(subject).join(" ")
}

/// Treść znormalizowana (deduplikacja): słowa złożone, małe litery, bez interpunkcji.
pub fn normalized_text(text: &str) -> String {
    search_tokens(text).join(" ")
}

/// Walidacja nowego wpisu F7 (wszystkie zakresy i warstwy).
pub fn validate_f7(new: &NewMemory) -> Result<(), MemoryError> {
    validate_scope(&new.scope)?;
    let text = new.text.trim();
    if text.is_empty() {
        return Err(MemoryError::invalid("pusta treść"));
    }
    if new.text.chars().count() > MAX_TEXT_CHARS {
        return Err(MemoryError::invalid(format!(
            "treść dłuższa niż {MAX_TEXT_CHARS} znaków"
        )));
    }
    if !(0.0..=1.0).contains(&new.confidence) {
        return Err(MemoryError::invalid(format!(
            "pewność {} poza 0–1",
            new.confidence
        )));
    }
    if new.layer == Layer::Working && !matches!(new.scope, MemoryScope::Session(_)) {
        return Err(MemoryError::invalid(
            "warstwa robocza istnieje tylko w zakresie sesji",
        ));
    }
    let label_ok = |s: &String| !s.trim().is_empty() && s.chars().count() <= MAX_LABEL_CHARS;
    if new.entities.len() > MAX_ENTITIES || !new.entities.iter().all(label_ok) {
        return Err(MemoryError::invalid("nieprawidłowe encje"));
    }
    if new.subject.as_ref().is_some_and(|s| !label_ok(s)) {
        return Err(MemoryError::invalid("nieprawidłowy temat"));
    }
    if new
        .origin
        .derived_from
        .iter()
        .any(|r| is_broader(&r.scope, &new.scope))
    {
        return Err(MemoryError::invalid(
            "źródło wpisu nie może leżeć w zakresie szerszym niż wpis",
        ));
    }
    Ok(())
}

impl<B: MemoryBackend> MemoryService for MemoryEngine<B> {
    fn remember_as(
        &self,
        who: &Accessor,
        new: NewMemory,
        mode: RememberMode,
    ) -> Result<MemoryEntry, MemoryError> {
        self.do_remember(who, new, mode)
    }

    fn recall_as(
        &self,
        who: &Accessor,
        request: &RecallRequest,
    ) -> Result<Vec<Recalled>, MemoryError> {
        self.do_recall(who, request)
    }

    fn get_as(&self, who: &Accessor, entry: &EntryRef) -> Result<MemoryEntry, MemoryError> {
        self.do_get(who, entry)
    }

    fn working_set(
        &self,
        who: &Accessor,
        session: &SessionId,
        query: Option<&str>,
        budget_chars: usize,
    ) -> Result<WorkingSet, MemoryError> {
        self.do_working_set(who, session, query, budget_chars)
    }

    fn set_pinned(
        &self,
        who: &Accessor,
        entry: &EntryRef,
        pinned: bool,
    ) -> Result<MemoryEntry, MemoryError> {
        self.do_set_pinned(who, entry, pinned)
    }

    fn approve_as(&self, who: &Accessor, entry: &EntryRef) -> Result<MemoryEntry, MemoryError> {
        self.do_approve(who, entry)
    }

    fn promote_as(
        &self,
        who: &Accessor,
        entry: &EntryRef,
        to: MemoryScope,
    ) -> Result<MemoryEntry, MemoryError> {
        self.do_promote(who, entry, to)
    }

    fn forget_as(
        &self,
        who: &Accessor,
        target: &ForgetTarget,
    ) -> Result<CascadeReport, MemoryError> {
        self.do_forget(who, target)
    }

    fn inspect(
        &self,
        who: &Accessor,
        query: &InspectorQuery,
    ) -> Result<InspectorPage, MemoryError> {
        self.do_inspect(who, query)
    }

    fn explain(&self, who: &Accessor, entry: &EntryRef) -> Result<Explanation, MemoryError> {
        self.do_explain(who, entry)
    }

    fn edit(
        &self,
        who: &Accessor,
        entry: &EntryRef,
        edit: &EntryEdit,
    ) -> Result<MemoryEntry, MemoryError> {
        self.do_edit(who, entry, edit)
    }

    fn scopes(&self, who: &Accessor) -> Result<Vec<ScopeSummary>, MemoryError> {
        self.do_scopes(who)
    }

    fn export_scope(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
        name: &str,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        self.do_export(who, scope, name)
    }

    fn import_scope(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
        entries: Vec<MemoryEntry>,
        policy: ImportPolicy,
    ) -> Result<ImportReport, MemoryError> {
        self.do_import(who, scope, entries, policy)
    }

    fn apply_changes(&self, who: &Accessor, set: &ChangeSet) -> Result<ChangeReport, MemoryError> {
        self.do_apply_changes(who, set)
    }

    fn journal(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
    ) -> Result<Vec<JournalRecord>, MemoryError> {
        crate::access::require_owner_or_guardian(who, "dziennik pamięci")?;
        validate_scope(scope)?;
        self.backend.journal(scope)
    }

    fn undo(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
        change: &ChangeId,
    ) -> Result<UndoReport, MemoryError> {
        self.do_undo(who, scope, change)
    }
}
