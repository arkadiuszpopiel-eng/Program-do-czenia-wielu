//! Implementacja modułu `search` (docs/modules/search/SPEC.md, ADR 0008).
//!
//! Indeks żyje w szyfrowanej bazie sesji (tabele `search_docs`, `search_fts`, `search_vec_<rodzaj>`),
//! którą udostępnia `sessions` przez [`SessionDbProvider`]. Tury są indeksowane w transakcji
//! `append_turn` przez [`TxIndexer`] (ten sam obiekt [`SqliteSearch`]); `memory` robi to samo przy
//! `remember`/`forget`.
//!
//! Wyszukiwanie między sesjami (`SessionSet::Many/All`) to funkcja UI właściciela — otwiera wiele
//! baz przez dostawcę; agentka dostaje wyłącznie własną sesję (`search_contract::authorize`).
//!
//! [`TxSearcher`] pyta w połączeniu modułu-właściciela bazy (np. `memory` w bazie zakresu globalnego
//! lub projektu) — FTS z dopasowaniem „dowolne słowo” dla recall pamięci; [`TxIndexer::compact_in`]
//! scala segmenty FTS5, żeby słowa usuniętych dokumentów nie zostały w tabelach indeksu.
//!
//! **Zmiana embeddera** (F7-02): identyfikator embeddera jest zapisany w bazie; inny embedder →
//! przebudowa wektorów do nowej generacji tabel `vec0` ([`state`]), w trakcie zapytania wektorowe
//! i hybrydowe działają jak FTS. Kroki przebudowy ([`TxIndexer::reindex_step`]) liczą embeddingi poza
//! blokadą bazy; [`SqliteSearch::spawn_reindex`] przechodzi po bazach w tle (wznawialnie, z postępem).
//! Embedder niedostępny przy zapisie → dokument w FTS, wektor uzupełni przebudowa.
//! Embedder wymienia się w działającej usłudze ([`SqliteSearch::set_embedder`] — wybór modelu w UI);
//! każda operacja bierze migawkę embeddera na początku, więc stan bazy i wektory jednej operacji
//! zawsze pochodzą od tego samego modelu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod events;
mod index;
mod query;
mod reindex;
mod state;
mod worker;

use std::sync::{Arc, PoisonError, RwLock};
use std::time::Instant;

use core_bus_contract::Level;
use core_registry_contract::{ManifestError, ModuleManifest};
use lib_sqlstore::Db;
use lib_sqlstore::rusqlite::Connection;
use search_contract::{
    Caller, ConnQuery, DEFAULT_SNIPPET_CHARS, Doc, DocId, Embedder, Hit, MAX_LIMIT, Query,
    ReindexProgress, RemoveReport, Search, SearchError, SessionId, SessionSet, TxIndexer,
    TxSearcher, VectorStatus, authorize, events as names, sort_hits,
};
use serde_json::json;
use sessions_contract::{SessionDbProvider, SessionError};

use crate::events::Outbox;
use crate::state::VecState;

pub use state::{MIGRATIONS, NAMESPACE, vec_table, vec_table_gen};
pub use worker::{ProgressFn, ReindexHandle, ReindexOptions, ReindexReport, ReindexSource};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Wyszukiwanie na bazach sesji (SQLCipher + FTS5 + sqlite-vec).
pub struct SqliteSearch {
    provider: Arc<dyn SessionDbProvider>,
    embedder: RwLock<Arc<dyn Embedder>>,
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
            embedder: RwLock::new(embedder),
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

    /// Bieżący embedder (migawka — operacja w toku zostaje przy swoim).
    pub fn embedder(&self) -> Arc<dyn Embedder> {
        self.embedder
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Wymienia embedder (np. model wybrany w UI). Kolejne operacje widzą nowy `model_id` → bazy
    /// zaczynają przebudowę wektorów ([`SqliteSearch::spawn_reindex`]); do jej końca zapytania
    /// wektorowe i hybrydowe działają jak FTS.
    pub fn set_embedder(&self, embedder: Arc<dyn Embedder>) {
        *self
            .embedder
            .write()
            .unwrap_or_else(PoisonError::into_inner) = embedder;
    }

    fn db(&self, session: &SessionId) -> Result<Arc<Db>, SearchError> {
        self.provider
            .session_db(session)
            .map_err(|e| session_err(session, e))
    }

    /// Stan wektorów bazy; początek przebudowy → zdarzenie `search.reindex.started`.
    fn prepare_conn(
        &self,
        conn: &Connection,
        embedder: &dyn Embedder,
        label: Option<&SessionId>,
    ) -> Result<VecState, SearchError> {
        let state = state::prepare(conn, embedder)?;
        if state.just_started {
            let to = state.target.as_ref().map(|t| t.1.clone());
            let payload = json!({ "from": state.active, "to": to });
            self.outbox
                .emit(names::REINDEX_STARTED, Level::Info, label, payload);
        }
        Ok(state)
    }

    fn index_with(
        &self,
        conn: &Connection,
        embedder: &dyn Embedder,
        state: &VecState,
        doc: &Doc,
    ) -> Result<(), SearchError> {
        if !index::index_doc(conn, embedder, state, doc)? {
            let payload = json!({ "kind": doc.id.kind.as_str() });
            self.outbox.emit(
                names::VECTOR_MISSING,
                Level::Warn,
                Some(&doc.session),
                payload,
            );
        }
        Ok(())
    }

    fn in_tx<R>(
        &self,
        session: &SessionId,
        f: impl FnOnce(&Connection, &dyn Embedder, &VecState) -> Result<R, SearchError>,
    ) -> Result<R, SearchError> {
        let embedder = self.embedder();
        self.db(session)?.with(|conn| {
            let tx = conn.transaction().map_err(index::storage)?;
            let state = self.prepare_conn(&tx, embedder.as_ref(), Some(session))?;
            let out = f(&tx, embedder.as_ref(), &state)?;
            tx.commit().map_err(index::storage)?;
            Ok(out)
        })
    }

    /// Stan wektorów sesji (UI: „reindeksacja X/Y”, „brak wektorów”).
    pub fn vector_status(&self, session: &SessionId) -> Result<VectorStatus, SearchError> {
        let embedder = self.embedder();
        self.db(session)?.with(|conn| {
            let state = self.prepare_conn(conn, embedder.as_ref(), Some(session))?;
            reindex::status(conn, &state)
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
        self.in_tx(&doc.session, |c, e, state| {
            self.index_with(c, e, state, doc)
        })
    }

    fn remove(&self, session: &SessionId, id: &DocId) -> Result<RemoveReport, SearchError> {
        let report = self.in_tx(session, |c, _, state| index::remove_doc(c, state, id))?;
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
        let embedder = self.embedder();
        let mut hits = Vec::new();
        for session in &sessions {
            let found = self.db(session)?.with(|conn| {
                let state = self.prepare_conn(conn, embedder.as_ref(), Some(session))?;
                query::query_conn(
                    conn,
                    embedder.as_ref(),
                    &state,
                    session,
                    &query::Parts::of_query(query),
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
        self.prepare_conn(conn, self.embedder().as_ref(), None)
            .map(|_| ())
    }

    fn index_in(&self, conn: &Connection, doc: &Doc) -> Result<(), SearchError> {
        let embedder = self.embedder();
        let state = self.prepare_conn(conn, embedder.as_ref(), Some(&doc.session))?;
        self.index_with(conn, embedder.as_ref(), &state, doc)
    }

    fn remove_in(
        &self,
        conn: &Connection,
        session: &SessionId,
        id: &DocId,
    ) -> Result<RemoveReport, SearchError> {
        let state = self.prepare_conn(conn, self.embedder().as_ref(), Some(session))?;
        index::remove_doc(conn, &state, id)
    }

    fn compact_in(&self, conn: &Connection) -> Result<(), SearchError> {
        self.prepare_conn(conn, self.embedder().as_ref(), None)?;
        index::compact(conn)
    }

    fn vector_status_in(&self, conn: &Connection) -> Result<VectorStatus, SearchError> {
        let state = self.prepare_conn(conn, self.embedder().as_ref(), None)?;
        reindex::status(conn, &state)
    }

    fn reindex_step(&self, db: &Db, batch: usize) -> Result<ReindexProgress, SearchError> {
        reindex::step(db, self.embedder().as_ref(), batch)
    }
}

impl TxSearcher for SqliteSearch {
    fn query_in(
        &self,
        conn: &Connection,
        label: &SessionId,
        query: &ConnQuery,
    ) -> Result<Vec<Hit>, SearchError> {
        let limit = query.limit.min(MAX_LIMIT);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let embedder = self.embedder();
        let state = self.prepare_conn(conn, embedder.as_ref(), Some(label))?;
        let mut hits = query::query_conn(
            conn,
            embedder.as_ref(),
            &state,
            label,
            &query::Parts::of_conn(query),
            limit,
            self.snippet_chars,
        )?;
        sort_hits(&mut hits);
        Ok(hits)
    }
}
