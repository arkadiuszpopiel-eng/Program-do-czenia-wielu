//! Kontrakt modułu `memory` (docs/modules/memory/SPEC.md, PLAN §10, §8.7, ADR 0008).
//!
//! **v0** ([`Memory`]): `remember/recall/forget` w zakresie sesji — zostaje jako fasada.
//!
//! **F7** ([`MemoryService`]): cztery warstwy ([`Layer`]: robocza, epizodyczna, semantyczna,
//! proceduralna), zakresy sesja/projekt/globalna/agentka z uprawnieniami wywołującego
//! ([`Accessor`], [`authorize`]), proweniencja ([`Provenance`], [`Origin`]), wersjonowanie faktów
//! (sprzeczność → nowa wersja z odwołaniem), TTL/retencja, prywatność (sesja prywatna nie zasila
//! zakresów szerszych; treść niezaufana nie awansuje), Inspektor ([`InspectorQuery`],
//! [`Explanation`]), `forget` kaskadowo ([`ForgetTarget`], [`CascadeReport`]), dziennik zmian z
//! cofaniem ([`ChangeSet`], [`JournalRecord`]), recall hybrydowy z rerankingiem ([`Reranker`]).
//!
//! Cała logika F7 jest w [`MemoryEngine`] nad portem magazynu [`MemoryBackend`] — `memory-impl`
//! (SQLCipher + `search`) i `memory-fake` (mapy) różnią się tylko magazynem.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod access;
mod backend;
mod cascade;
mod engine;
mod error;
pub mod export;
mod inspect;
mod journal;
mod model;
mod ports;
pub mod rerank;
mod rules;
mod service;
mod types;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
/// Współdzielone testy kontraktowe F7 ([`MemoryService`]) — `-impl` i `-fake`.
#[cfg(feature = "contract-tests")]
pub mod contract_tests_f7;

pub use access::{
    Accessor, AgentAccess, Op, ScopeGrant, authorize, readable_scopes, require_owner,
    require_owner_or_guardian,
};
pub use backend::{
    Candidate, CandidateQuery, CommitReport, DropReport, ExportNote, MemoryBackend, StoreOp,
};
pub use cascade::{CascadePlan, plan_cascade};
pub use engine::{
    CACHE_CAPACITY, CACHE_TTL_SECS, EnginePorts, MAX_ENTITIES, MAX_ID_LEN, MAX_LABEL_CHARS,
    MAX_TEXT_CHARS, MemoryEngine, candidate_limit, check_imported, normalized_text, subject_key,
    trust_rank, validate_f7,
};
pub use error::MemoryError;
pub use inspect::{
    DEFAULT_PAGE, EntryEdit, Explanation, InspectorItem, InspectorPage, InspectorQuery, MAX_PAGE,
    SourceLink,
};
pub use journal::{
    ChangeId, ChangeKind, ChangeOp, ChangeReport, ChangeSet, JournalRecord, UndoReport,
};
pub use model::{
    Derivation, EntryRef, EntryState, MAX_SCOPE_ID_LEN, Origin, SupersedeReason, Supersession,
    entry_state, expires_at, is_broader, is_recallable, parse_scope_key, scope_key, scope_session,
    validate_scope,
};
pub use ports::{
    EventSink, IdSource, MemoryClock, NoEvents, PrivacyOracle, PrivateSessions, RecordingEvents,
    SeqIds, SystemClock, VirtualClock,
};
pub use rerank::{HeuristicReranker, RerankItem, Reranker};
pub use rules::{check_promotion, is_expired, recall_sessions, validate_new};
pub use service::{
    CascadeReport, ForgetTarget, ImportPolicy, ImportReport, MemoryService, RecallRequest,
    ScopeSummary, WorkingSet,
};
pub use types::{
    ForgetReport, Layer, Memory, MemoryEntry, MemoryId, MemoryScope, NewMemory, Provenance,
    Recalled, RememberMode,
};

pub use core_bus_contract::{AgentId, SessionId};

/// Nazwy zdarzeń modułu (ładunki bez treści wpisów — tylko identyfikatory i liczniki).
pub mod events {
    /// Zapamiętano wpis (`{ "memory", "layer", "trusted", "approved" }`).
    pub const REMEMBERED: &str = "memory.remembered";
    /// Wpis czeka na zatwierdzenie użytkownika.
    pub const PENDING_APPROVAL: &str = "memory.pending_approval";
    /// `recall` wykonany (Diagnostics: liczba zakresów, liczba wyników).
    pub const RECALLED: &str = "memory.recalled";
    /// Wpis zapomniany (raport kaskady).
    pub const FORGOTTEN: &str = "memory.forgotten";
    /// Wpis zatwierdzony przez użytkownika (F7).
    pub const APPROVED: &str = "memory.approved";
    /// Wpis przypięty/odpięty (F7).
    pub const PINNED: &str = "memory.pinned";
    /// Nowa wersja po edycji (F7).
    pub const EDITED: &str = "memory.edited";
    /// Awans do zakresu szerszego (F7).
    pub const PROMOTED: &str = "memory.promoted";
    /// Zastosowano zmiany konsolidacji (F7).
    pub const CHANGES_APPLIED: &str = "memory.changes.applied";
    /// Cofnięto zmianę z dziennika (F7).
    pub const CHANGE_UNDONE: &str = "memory.change.undone";
    /// Wyeksportowano zakres (F7).
    pub const EXPORTED: &str = "memory.exported";
    /// Zaimportowano zakres (F7).
    pub const IMPORTED: &str = "memory.imported";
    /// Konsolidacja rozpoczęta (F7, Strażniczka pamięci).
    pub const CONSOLIDATION_STARTED: &str = "memory.consolidation.started";
    /// Konsolidacja zakończona (F7).
    pub const CONSOLIDATION_FINISHED: &str = "memory.consolidation.finished";
    /// Konsolidacja pominięta (bateria, tryb gry, okno, budżet; F7).
    pub const CONSOLIDATION_SKIPPED: &str = "memory.consolidation.skipped";
}
