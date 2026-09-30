//! Implementacja modułu `search` (docs/modules/search/SPEC.md, ADR 0008).
//!
//! Indeks żyje w szyfrowanej bazie sesji (tabele `search_docs`, `search_fts`, `search_vec_<rodzaj>`),
//! którą udostępnia `sessions` przez [`SessionDbProvider`]. Tury są indeksowane w transakcji
//! `append_turn` przez [`TxIndexer`] (ten sam obiekt [`SqliteSearch`]); `memory` robi to samo przy
//! `remember`/`forget`.
//!
//! Wyszukiwanie między sesjami (`SessionSet::Many/All`) to funkcja UI właściciela — otwiera wiele
//! baz przez dostawcę; agentka dostaje wyłącznie własną sesję (`search_contract::authorize`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod events;
mod index;
mod query;

use std::sync::Arc;
use std::time::Instant;

use core_bus_contract::Level;
use core_registry_contract::{ManifestError, ModuleManifest};
use lib_sqlstore::Db;
use lib_sqlstore::rusqlite::Connection;
use search_contract::{
    Caller, DEFAULT_SNIPPET_CHARS, Doc, DocId, Embedder, Hit, MAX_LIMIT, Query, RemoveReport,
    Search, SearchError, SessionId, SessionSet, TxIndexer, authorize, events as names, sort_hits,
};
use serde_json::json;
use sessions_contract::{SessionDbProvider, SessionError};

use crate::events::Outbox;

pub use index::{MIGRATIONS, NAMESPACE, vec_table};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Wyszukiwanie na bazach sesji (SQLCipher + FTS5 + sqlite-vec).
pub struct SqliteSearch {
    provider: Arc<dyn SessionDbProvider>,
    embedder: Arc<dyn Embedder>,
    snippet_chars: usize,
    outbox: Outbox,
    manifest: ModuleManifest,
}

fn session_err(session: &SessionId, e: SessionError) -> SearchError {
    match e {
        SessionError::NotFound { .. } => SearchError::SessionNotFound {
            session: session.to_string(),
        },
        other => SearchError::storage(other),
    }
}

impl SqliteSearch {
    /// Nowa usługa; `embedder` lokalny (w testach `search_fake::HashEmbedder`).
    pub fn new(
        provider: Arc<dyn SessionDbProvider>,
        embedder: Arc<dyn Embedder>,
    ) -> Result<Self, ManifestError> {
        Ok(Self {
            provider,
            embedder,
            snippet_chars: DEFAULT_SNIPPET_CHARS,
            outbox: Outbox::default(),
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
        })
    }

    /// Długość fragmentu (`[search] snippet_chars`).
    #[must_use]
    pub fn with_snippet_chars(mut self, chars: usize) -> Self {
        self.snippet_chars = chars;
        self
    }

    fn db(&self, session: &SessionId) -> Result<Arc<Db>, SearchError> {
        self.provider
            .session_db(session)
            .map_err(|e| session_err(session, e))
    }

    fn in_tx<R>(
        &self,
        session: &SessionId,
        f: impl FnOnce(&Connection) -> Result<R, SearchError>,
    ) -> Result<R, SearchError> {
        self.db(session)?.with(|conn| {
            let tx = conn.transaction().map_err(index::storage)?;
            index::prepare(&tx, self.embedder.as_ref())?;
            let out = f(&tx)?;
            tx.commit().map_err(index::storage)?;
            Ok(out)
        })
    }

    fn sessions_of(&self, set: &SessionSet) -> Result<Vec<SessionId>, SearchError> {
        let mut ids = match set {
            SessionSet::One(s) => vec![s.clone()],
            SessionSet::Many(list) => list.clone(),
            SessionSet::All => self.provider.session_ids().map_err(SearchError::storage)?,
        };
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

impl Search for SqliteSearch {
    fn index(&self, doc: &Doc) -> Result<(), SearchError> {
        self.in_tx(&doc.session, |c| {
            index::index_doc(c, self.embedder.as_ref(), doc)
        })
    }

    fn remove(&self, session: &SessionId, id: &DocId) -> Result<RemoveReport, SearchError> {
        let report = self.in_tx(session, |c| index::remove_doc(c, id))?;
        let payload =
            json!({ "docs": report.docs, "fts_rows": report.fts_rows, "vectors": report.vectors });
        self.outbox
            .emit(names::REMOVED, Level::Info, Some(session), payload);
        Ok(report)
    }

    fn query(&self, query: &Query, caller: &Caller) -> Result<Vec<Hit>, SearchError> {
        authorize(&query.sessions, caller)?;
        let limit = query.limit.min(MAX_LIMIT);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let started = Instant::now();
        let sessions = self.sessions_of(&query.sessions)?;
        let mut hits = Vec::new();
        for session in &sessions {
            let found = self.db(session)?.with(|conn| {
                index::prepare(conn, self.embedder.as_ref())?;
                query::query_conn(
                    conn,
                    self.embedder.as_ref(),
                    session,
                    query,
                    limit,
                    self.snippet_chars,
                )
            })?;
            hits.extend(found);
        }
        sort_hits(&mut hits);
        hits.truncate(limit);
        let payload = json!({
            "mode": query.mode, "sessions": sessions.len(), "hits": hits.len(),
            "ms": started.elapsed().as_millis(),
        });
        self.outbox.emit(names::QUERY, Level::Debug, None, payload);
        Ok(hits)
    }
}

impl TxIndexer for SqliteSearch {
    fn prepare(&self, conn: &Connection) -> Result<(), SearchError> {
        index::prepare(conn, self.embedder.as_ref())
    }

    fn index_in(&self, conn: &Connection, doc: &Doc) -> Result<(), SearchError> {
        index::prepare(conn, self.embedder.as_ref())?;
        index::index_doc(conn, self.embedder.as_ref(), doc)
    }

    fn remove_in(
        &self,
        conn: &Connection,
        _session: &SessionId,
        id: &DocId,
    ) -> Result<RemoveReport, SearchError> {
        index::prepare(conn, self.embedder.as_ref())?;
        index::remove_doc(conn, id)
    }
}
