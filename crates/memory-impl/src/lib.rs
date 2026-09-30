//! Implementacja modułu `memory` v0 (docs/modules/memory/SPEC.md, PLAN §10, ADR 0008).
//!
//! Wpisy zakresu `sesja` żyją w tabeli `memory_entries` **szyfrowanej bazy sesji**
//! ([`SessionDbProvider`]); indeks FTS + wektor to dokumenty `DocKind::Memory` modułu `search`,
//! zapisywane w tej samej transakcji ([`TxIndexer`]). `recall` = hybryda (RRF) przez [`Search`]
//! wywoływany jako agentka tej sesji (najmniejsze uprawnienia). `forget` usuwa wpis, wiersz FTS i
//! wektor w jednej transakcji i zwraca raport kaskady.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod events;
mod store;

use std::sync::Arc;

use chrono::Utc;
use core_bus_contract::Level;
use core_registry_contract::{ManifestError, ModuleManifest};
use memory_contract::{
    ForgetReport, Memory, MemoryEntry, MemoryError, MemoryId, MemoryScope, NewMemory, Recalled,
    RememberMode, SessionId, check_promotion, events as names, is_expired, recall_sessions,
    validate_new,
};
use search_contract::{Caller, DocKind, Mode, Query, Search, SessionSet, TxIndexer};
use serde_json::json;
use sessions_contract::SessionDbProvider;

use crate::events::Outbox;

pub use store::{MIGRATIONS, NAMESPACE};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Pamięć v0 na bazach sesji.
pub struct SqliteMemory {
    provider: Arc<dyn SessionDbProvider>,
    indexer: Arc<dyn TxIndexer>,
    search: Arc<dyn Search>,
    outbox: Outbox,
    manifest: ModuleManifest,
}

impl SqliteMemory {
    /// Nowa pamięć. W kompozycji `indexer` i `search` to ten sam `search_impl::SqliteSearch`.
    pub fn new(
        provider: Arc<dyn SessionDbProvider>,
        indexer: Arc<dyn TxIndexer>,
        search: Arc<dyn Search>,
    ) -> Result<Self, ManifestError> {
        Ok(Self {
            provider,
            indexer,
            search,
            outbox: Outbox::default(),
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
        })
    }

    fn store(&self, session: &SessionId) -> Result<store::Store<'_>, MemoryError> {
        let db = self
            .provider
            .session_db(session)
            .map_err(MemoryError::storage)?;
        Ok(store::Store::new(
            db,
            self.indexer.as_ref(),
            session.clone(),
        ))
    }

    fn session_of(scope: &MemoryScope) -> Result<SessionId, MemoryError> {
        recall_sessions(std::slice::from_ref(scope))?
            .into_iter()
            .next()
            .ok_or_else(|| MemoryError::Invalid {
                reason: "brak zakresu".into(),
            })
    }

    fn recall_in(
        &self,
        session: &SessionId,
        query: &str,
        k: usize,
    ) -> Result<Vec<Recalled>, MemoryError> {
        let q = Query {
            text: query.to_owned(),
            sessions: SessionSet::One(session.clone()),
            mode: Mode::Hybrid,
            limit: (k * 4).max(20),
            kinds: vec![DocKind::Memory],
        };
        let caller = Caller::Agent {
            session: session.clone(),
        };
        let hits = self
            .search
            .query(&q, &caller)
            .map_err(MemoryError::storage)?;
        let store = self.store(session)?;
        let now = Utc::now();
        let mut out = Vec::new();
        for hit in hits {
            let id = MemoryId(hit.doc.key);
            if let Some(entry) = store.load(&id)?
                && entry.approved
                && !is_expired(&entry, now)
            {
                out.push(Recalled {
                    entry,
                    score: hit.score,
                });
            }
        }
        Ok(out)
    }
}

impl Memory for SqliteMemory {
    fn remember(&self, new: NewMemory, mode: RememberMode) -> Result<MemoryEntry, MemoryError> {
        let session = validate_new(&new, mode)?;
        let entry = MemoryEntry {
            id: MemoryId(uuid::Uuid::now_v7().to_string()),
            trusted: new.provenance.is_trusted(),
            scope: new.scope,
            layer: new.layer,
            text: new.text,
            entities: new.entities,
            provenance: new.provenance,
            confidence: new.confidence,
            ttl_secs: new.ttl_secs,
            created_at: Utc::now(),
            approved: mode == RememberMode::Explicit,
        };
        self.store(&session)?.insert(&entry)?;
        let payload = json!({
            "memory": entry.id, "layer": entry.layer, "trusted": entry.trusted, "approved": entry.approved,
        });
        let kind = if entry.approved {
            names::REMEMBERED
        } else {
            names::PENDING_APPROVAL
        };
        self.outbox.emit(kind, Level::Info, Some(&session), payload);
        Ok(entry)
    }

    fn recall(
        &self,
        scopes: &[MemoryScope],
        query: &str,
        k: usize,
    ) -> Result<Vec<Recalled>, MemoryError> {
        let sessions = recall_sessions(scopes)?;
        if k == 0 || query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for session in &sessions {
            out.extend(self.recall_in(session, query, k)?);
        }
        out.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.entry.id.cmp(&b.entry.id))
        });
        out.truncate(k);
        let payload = json!({ "scopes": sessions.len(), "results": out.len() });
        self.outbox
            .emit(names::RECALLED, Level::Debug, None, payload);
        Ok(out)
    }

    fn get(&self, scope: &MemoryScope, id: &MemoryId) -> Result<MemoryEntry, MemoryError> {
        let session = Self::session_of(scope)?;
        self.store(&session)?
            .load(id)?
            .ok_or_else(|| MemoryError::NotFound { id: id.clone() })
    }

    fn list(&self, scope: &MemoryScope) -> Result<Vec<MemoryEntry>, MemoryError> {
        let session = Self::session_of(scope)?;
        self.store(&session)?.list()
    }

    fn approve(&self, scope: &MemoryScope, id: &MemoryId) -> Result<MemoryEntry, MemoryError> {
        let session = Self::session_of(scope)?;
        self.store(&session)?.approve(id)
    }

    fn forget(&self, scope: &MemoryScope, id: &MemoryId) -> Result<ForgetReport, MemoryError> {
        let session = Self::session_of(scope)?;
        let report = self.store(&session)?.forget(id)?;
        let payload = json!({
            "memory": id, "fts_rows": report.fts_rows, "vectors": report.vectors, "derived": report.derived,
        });
        self.outbox
            .emit(names::FORGOTTEN, Level::Info, Some(&session), payload);
        Ok(report)
    }

    fn promote(
        &self,
        scope: &MemoryScope,
        id: &MemoryId,
        to: MemoryScope,
    ) -> Result<MemoryEntry, MemoryError> {
        let entry = self.get(scope, id)?;
        check_promotion(&entry, &to)?;
        Ok(entry)
    }
}
