//! Magazyn F7 na szyfrowanych bazach SQLite (`MemoryBackend`): wpisy (`memory_entries`), dziennik
//! (`memory_journal`), notatki eksportów (`memory_exports`) i indeks `search` (FTS5 + `vec0`) w tej
//! samej transakcji. Po usunięciach: `secure_delete`, kompakcja FTS (`TxIndexer::compact_in`),
//! checkpoint WAL (`TRUNCATE`) — usunięta treść nie zostaje w stronach ani w segmentach indeksu.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use lib_sqlstore::Db;
use lib_sqlstore::rusqlite::{Connection, OptionalExtension, params};
use memory_contract::{
    Candidate, CandidateQuery, CommitReport, DropReport, ExportNote, JournalRecord, MemoryBackend,
    MemoryEntry, MemoryError, MemoryId, MemoryScope, StoreOp, scope_key,
};
use search_contract::{ConnQuery, Doc, DocKind, Mode, SessionId, TxIndexer, TxSearcher};

use crate::scopes::ScopeDbs;
use crate::store::{doc_id, prepare, storage};

/// Etykieta bazy w indeksie: sesja → jej identyfikator; zakres własny → `@<klucz zakresu>`.
pub fn index_label(scope: &MemoryScope) -> SessionId {
    match scope {
        MemoryScope::Session(s) => s.clone(),
        other => SessionId::new(format!("@{}", scope_key(other))),
    }
}

/// Magazyn F7 na bazach zakresów.
pub struct SqliteBackend {
    dbs: Arc<dyn ScopeDbs>,
    indexer: Arc<dyn TxIndexer>,
    searcher: Arc<dyn TxSearcher>,
}

fn parse<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, MemoryError> {
    serde_json::from_str(body).map_err(storage)
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, MemoryError> {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![name],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .map_err(storage)
}

fn bodies(conn: &Connection, sql: &str) -> Result<Vec<String>, MemoryError> {
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    stmt.query_map([], |r| r.get::<_, String>(0))
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)
}

impl SqliteBackend {
    /// Nowy magazyn. W kompozycji `indexer` i `searcher` to ten sam `search_impl::SqliteSearch`.
    pub fn new(
        dbs: Arc<dyn ScopeDbs>,
        indexer: Arc<dyn TxIndexer>,
        searcher: Arc<dyn TxSearcher>,
    ) -> Self {
        Self {
            dbs,
            indexer,
            searcher,
        }
    }

    /// Bazy zakresów (kompozycja: przebudowa wektorów po zmianie embeddera w bazach zakresów).
    pub fn dbs(&self) -> &Arc<dyn ScopeDbs> {
        &self.dbs
    }

    fn read<R: Default>(
        &self,
        scope: &MemoryScope,
        f: impl FnOnce(&Connection) -> Result<R, MemoryError>,
    ) -> Result<R, MemoryError> {
        match self.dbs.db(scope, false)? {
            Some(db) => db.with(|conn| {
                prepare(conn, self.indexer.as_ref())?;
                f(conn)
            }),
            None => Ok(R::default()),
        }
    }

    fn apply(
        &self,
        conn: &Connection,
        label: &SessionId,
        op: StoreOp,
        report: &mut CommitReport,
    ) -> Result<(), MemoryError> {
        match op {
            StoreOp::Put(e) => {
                let old: Option<String> = conn
                    .query_row(
                        "SELECT body FROM memory_entries WHERE id = ?1",
                        params![e.id.0],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(storage)?;
                let old_text = old
                    .map(|b| parse::<MemoryEntry>(&b))
                    .transpose()?
                    .map(|o| o.text);
                let body = serde_json::to_string(&e).map_err(storage)?;
                conn.execute(
                    "INSERT OR REPLACE INTO memory_entries(id, body, approved, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![e.id.0, body, e.approved, e.created_at.timestamp_millis()],
                )
                .map_err(storage)?;
                if old_text.as_deref() != Some(e.text.as_str()) {
                    let doc = Doc {
                        id: doc_id(&e.id),
                        session: label.clone(),
                        text: e.text.clone(),
                        ts: e.created_at,
                    };
                    self.indexer.index_in(conn, &doc).map_err(storage)?;
                }
            }
            StoreOp::Delete(id) => {
                let n = conn
                    .execute("DELETE FROM memory_entries WHERE id = ?1", params![id.0])
                    .map_err(storage)?;
                if n > 0 {
                    let r = self
                        .indexer
                        .remove_in(conn, label, &doc_id(&id))
                        .map_err(storage)?;
                    report.entries_deleted += n;
                    report.fts_rows += r.fts_rows;
                    report.vectors += r.vectors;
                }
            }
            StoreOp::PutJournal(rec) => {
                let body = serde_json::to_string(&rec).map_err(storage)?;
                conn.execute(
                    "INSERT OR REPLACE INTO memory_journal(id, at, run, body) VALUES (?1, ?2, ?3, ?4)",
                    params![rec.id.0, rec.at.timestamp_millis(), rec.run, body],
                )
                .map_err(storage)?;
            }
            StoreOp::DeleteJournal(id) => {
                report.journal_deleted += conn
                    .execute("DELETE FROM memory_journal WHERE id = ?1", params![id.0])
                    .map_err(storage)?;
            }
            StoreOp::NoteExport { name, at } => {
                conn.execute(
                    "INSERT OR REPLACE INTO memory_exports(name, at) VALUES (?1, ?2)",
                    params![name, at.timestamp_millis()],
                )
                .map_err(storage)?;
            }
        }
        Ok(())
    }

    /// Zatarcie po usunięciach: kompakcja indeksu FTS i checkpoint WAL (poza transakcją).
    fn scrub(&self, conn: &Connection) -> Result<(), MemoryError> {
        self.indexer.compact_in(conn).map_err(storage)?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(storage)
    }

    fn has_data(db: &Db) -> Result<bool, MemoryError> {
        db.with(|conn| {
            for table in ["memory_entries", "memory_journal", "memory_exports"] {
                if table_exists(conn, table)? {
                    let any: i64 = conn
                        .query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table})"), [], |r| {
                            r.get(0)
                        })
                        .map_err(storage)?;
                    if any != 0 {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })
    }
}

impl MemoryBackend for SqliteBackend {
    fn entries(&self, scope: &MemoryScope) -> Result<Vec<MemoryEntry>, MemoryError> {
        self.read(scope, |conn| {
            bodies(
                conn,
                "SELECT body FROM memory_entries ORDER BY created_at, id",
            )?
            .iter()
            .map(|b| parse(b))
            .collect()
        })
    }

    fn entry(
        &self,
        scope: &MemoryScope,
        id: &MemoryId,
    ) -> Result<Option<MemoryEntry>, MemoryError> {
        self.read(scope, |conn| {
            let body: Option<String> = conn
                .query_row(
                    "SELECT body FROM memory_entries WHERE id = ?1",
                    params![id.0],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            body.map(|b| parse(&b)).transpose()
        })
    }

    fn search(
        &self,
        scope: &MemoryScope,
        query: &CandidateQuery,
    ) -> Result<Vec<Candidate>, MemoryError> {
        let label = index_label(scope);
        let q = ConnQuery {
            text: query.stems.join(" "),
            vector_text: Some(query.text.clone()),
            mode: Mode::Hybrid,
            limit: query.limit,
            kinds: vec![DocKind::Memory],
            match_any: true,
        };
        self.read(scope, |conn| {
            let hits = self.searcher.query_in(conn, &label, &q).map_err(storage)?;
            Ok(hits
                .into_iter()
                .map(|h| Candidate {
                    id: MemoryId(h.doc.key),
                    score: h.score,
                })
                .collect())
        })
    }

    fn commit(&self, scope: &MemoryScope, ops: Vec<StoreOp>) -> Result<CommitReport, MemoryError> {
        let db = self
            .dbs
            .db(scope, true)?
            .ok_or_else(|| MemoryError::storage("brak bazy zakresu"))?;
        let label = index_label(scope);
        let deletes = ops
            .iter()
            .any(|op| matches!(op, StoreOp::Delete(_) | StoreOp::DeleteJournal(_)));
        db.with(|conn| {
            if deletes {
                conn.execute_batch("PRAGMA secure_delete = ON;")
                    .map_err(storage)?;
            }
            let mut report = CommitReport::default();
            let tx = conn.transaction().map_err(storage)?;
            prepare(&tx, self.indexer.as_ref())?;
            for op in ops {
                self.apply(&tx, &label, op, &mut report)?;
            }
            tx.commit().map_err(storage)?;
            if deletes {
                self.scrub(conn)?;
            }
            Ok(report)
        })
    }

    fn journal(&self, scope: &MemoryScope) -> Result<Vec<JournalRecord>, MemoryError> {
        self.read(scope, |conn| {
            bodies(
                conn,
                "SELECT body FROM memory_journal ORDER BY at DESC, id DESC",
            )?
            .iter()
            .map(|b| parse(b))
            .collect()
        })
    }

    fn exports(&self, scope: &MemoryScope) -> Result<Vec<ExportNote>, MemoryError> {
        self.read(scope, |conn| {
            let mut stmt = conn
                .prepare("SELECT name, at FROM memory_exports ORDER BY name")
                .map_err(storage)?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                .map_err(storage)?;
            let mut out = Vec::new();
            for row in rows {
                let (name, at) = row.map_err(storage)?;
                let at = DateTime::<Utc>::from_timestamp_millis(at).unwrap_or_default();
                out.push(ExportNote { name, at });
            }
            Ok(out)
        })
    }

    fn drop_scope(&self, scope: &MemoryScope) -> Result<DropReport, MemoryError> {
        let entries = self.entries(scope)?;
        let journal = self.journal(scope)?.len();
        if !matches!(scope, MemoryScope::Session(_)) {
            let shredded = self.dbs.shred(scope)?;
            let n = entries.len();
            return Ok(DropReport {
                commit: CommitReport {
                    entries_deleted: n,
                    fts_rows: n,
                    vectors: n,
                    journal_deleted: journal,
                },
                shredded,
            });
        }
        let mut ops: Vec<StoreOp> = entries.into_iter().map(|e| StoreOp::Delete(e.id)).collect();
        ops.extend(
            self.journal(scope)?
                .into_iter()
                .map(|j| StoreOp::DeleteJournal(j.id)),
        );
        let mut commit = self.commit(scope, ops)?;
        commit.journal_deleted = journal;
        if let Some(db) = self.dbs.db(scope, false)? {
            db.with(|conn| {
                conn.execute("DELETE FROM memory_exports", [])
                    .map_err(storage)
            })?;
        }
        Ok(DropReport {
            commit,
            shredded: false,
        })
    }

    fn scopes(&self) -> Result<Vec<MemoryScope>, MemoryError> {
        let mut out = Vec::new();
        for scope in self.dbs.known()? {
            if let Some(db) = self.dbs.db(&scope, false)?
                && Self::has_data(&db)?
            {
                out.push(scope);
            }
        }
        Ok(out)
    }
}
