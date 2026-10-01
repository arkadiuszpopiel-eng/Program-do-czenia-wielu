//! Komendy Inspektora pamięci (`memory_*`, F7) — delegują do `app-memory::MemoryApp`
//! (operacje jako właściciel; zapis do zakresów szerszych z sesji prywatnej jest odrzucany).

use crate::core::AppCore;
use crate::dto::{
    ConsolidationReport, MemoryEdit, MemoryExplanation, MemoryForgetPreview, MemoryForgetReport,
    MemoryForgetTarget, MemoryItem, MemoryJournalEntry, MemoryPage, MemoryQuery, MemoryScopeInfo,
    MemoryScopeRef, MemoryStatus, MemoryUndoResult,
};
use crate::error::AppError;

impl AppCore {
    /// `memory_status`: porządkowanie, licznik bezczynności, oczekujące propozycje.
    pub async fn memory_status(&self) -> Result<MemoryStatus, AppError> {
        self.inner.memory.status()
    }

    /// `memory_scopes`: zakresy z danymi.
    pub async fn memory_scopes(&self) -> Result<Vec<MemoryScopeInfo>, AppError> {
        self.inner.memory.scopes()
    }

    /// `memory_inspect`: lista z filtrami i wyszukiwaniem.
    pub async fn memory_inspect(&self, query: MemoryQuery) -> Result<MemoryPage, AppError> {
        self.inner.memory.inspect(query)
    }

    /// `memory_explain`: „dlaczego to pamiętam", wersje, źródła, dziennik.
    pub async fn memory_explain(&self, entry_id: String) -> Result<MemoryExplanation, AppError> {
        self.inner.memory.explain(&entry_id)
    }

    /// `memory_edit`: edycja = nowa wersja.
    pub async fn memory_edit(
        &self,
        entry_id: String,
        edit: MemoryEdit,
    ) -> Result<MemoryItem, AppError> {
        self.inner.memory.edit(&entry_id, edit)
    }

    /// `memory_set_pinned`: przypięcie (warstwa robocza).
    pub async fn memory_set_pinned(
        &self,
        entry_id: String,
        pinned: bool,
    ) -> Result<MemoryItem, AppError> {
        self.inner.memory.set_pinned(&entry_id, pinned)
    }

    /// `memory_approve`: zatwierdzenie propozycji (agentki, porządkowanie).
    pub async fn memory_approve(&self, entry_id: String) -> Result<MemoryItem, AppError> {
        self.inner.memory.approve(&entry_id)
    }

    /// `memory_promote`: kopia w zakresie szerszym.
    pub async fn memory_promote(
        &self,
        entry_id: String,
        to: MemoryScopeRef,
    ) -> Result<MemoryItem, AppError> {
        self.inner.memory.promote(&entry_id, &to)
    }

    /// `memory_forget_preview`: co zniknie (kaskada) i co wróci.
    pub async fn memory_forget_preview(
        &self,
        target: MemoryForgetTarget,
    ) -> Result<MemoryForgetPreview, AppError> {
        self.inner.memory.forget_preview(&target)
    }

    /// `memory_forget`: zapomnienie kaskadowe.
    pub async fn memory_forget(
        &self,
        target: MemoryForgetTarget,
    ) -> Result<MemoryForgetReport, AppError> {
        self.inner.memory.forget(&target)
    }

    /// `memory_journal`: dziennik zmian zakresu.
    pub async fn memory_journal(&self, scope: String) -> Result<Vec<MemoryJournalEntry>, AppError> {
        self.inner.memory.journal(&scope)
    }

    /// `memory_undo`: cofnięcie zmiany z dziennika.
    pub async fn memory_undo(
        &self,
        scope: String,
        change_id: String,
    ) -> Result<MemoryUndoResult, AppError> {
        self.inner.memory.undo(&scope, &change_id)
    }

    /// `memory_consolidate_now`: „Uporządkuj teraz" (nie na baterii ani w trybie gry).
    pub async fn memory_consolidate_now(&self) -> Result<ConsolidationReport, AppError> {
        self.inner.memory.consolidate_now().await
    }
}
