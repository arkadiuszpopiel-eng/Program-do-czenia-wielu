//! Fasada kontraktu v0 ([`Memory`]) na silniku F7: tylko zakres sesji (API v0 nie niesie
//! tożsamości wywołującego, więc zakresy szersze są dostępne wyłącznie przez [`MemoryService`]).
//! Wywołania wykonuje właściciel (UI: „zapamiętaj”, Inspektor sesji).

use super::MemoryEngine;
use crate::access::Accessor;
use crate::backend::MemoryBackend;
use crate::error::MemoryError;
use crate::model::EntryRef;
use crate::rules::{check_promotion, recall_sessions, validate_new};
use crate::service::{ForgetTarget, MemoryService, RecallRequest};
use crate::types::{
    ForgetReport, Memory, MemoryEntry, MemoryId, MemoryScope, NewMemory, Recalled, RememberMode,
};

fn session_scope(scope: &MemoryScope) -> Result<MemoryScope, MemoryError> {
    recall_sessions(std::slice::from_ref(scope))?
        .into_iter()
        .next()
        .map(MemoryScope::Session)
        .ok_or_else(|| MemoryError::invalid("brak zakresu"))
}

impl<B: MemoryBackend> Memory for MemoryEngine<B> {
    fn remember(&self, new: NewMemory, mode: RememberMode) -> Result<MemoryEntry, MemoryError> {
        validate_new(&new, mode)?;
        self.remember_as(&Accessor::Owner, new, mode)
    }

    fn recall(
        &self,
        scopes: &[MemoryScope],
        query: &str,
        k: usize,
    ) -> Result<Vec<Recalled>, MemoryError> {
        let sessions = recall_sessions(scopes)?;
        if k == 0 || query.trim().is_empty() || sessions.is_empty() {
            return Ok(Vec::new());
        }
        let scopes = sessions.into_iter().map(MemoryScope::Session).collect();
        self.recall_as(&Accessor::Owner, &RecallRequest::new(scopes, query, k))
    }

    fn get(&self, scope: &MemoryScope, id: &MemoryId) -> Result<MemoryEntry, MemoryError> {
        let scope = session_scope(scope)?;
        self.get_as(&Accessor::Owner, &EntryRef::new(scope, id.clone()))
    }

    fn list(&self, scope: &MemoryScope) -> Result<Vec<MemoryEntry>, MemoryError> {
        let scope = session_scope(scope)?;
        self.backend.entries(&scope)
    }

    fn approve(&self, scope: &MemoryScope, id: &MemoryId) -> Result<MemoryEntry, MemoryError> {
        let scope = session_scope(scope)?;
        self.approve_as(&Accessor::Owner, &EntryRef::new(scope, id.clone()))
    }

    fn forget(&self, scope: &MemoryScope, id: &MemoryId) -> Result<ForgetReport, MemoryError> {
        let scope = session_scope(scope)?;
        let target = ForgetTarget::Entry(EntryRef::new(scope, id.clone()));
        let report = self.forget_as(&Accessor::Owner, &target)?;
        Ok(ForgetReport {
            entry: !report.removed.is_empty(),
            fts_rows: report.fts_rows,
            vectors: report.vectors,
            derived: report.derived.len() + report.versions.len(),
        })
    }

    fn promote(
        &self,
        scope: &MemoryScope,
        id: &MemoryId,
        to: MemoryScope,
    ) -> Result<MemoryEntry, MemoryError> {
        let entry = Memory::get(self, scope, id)?;
        check_promotion(&entry, &to)?;
        Ok(entry)
    }
}
