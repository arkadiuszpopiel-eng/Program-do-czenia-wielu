//! Tabela `memory_entries` w bazie sesji + indeks (FTS/wektor) w tej samej transakcji.

use std::sync::Arc;

use lib_sqlstore::rusqlite::{Connection, OptionalExtension, params};
use lib_sqlstore::{Db, migrate};
use memory_contract::{ForgetReport, MemoryEntry, MemoryError, MemoryId, SessionId};
use search_contract::{Doc, DocId, DocKind, TxIndexer};

/// Przestrzeń nazw migracji.
pub const NAMESPACE: &str = "memory";

/// Migracje: wpis jako JSON (`body`) + kolumny do filtrowania i porządku.
pub const MIGRATIONS: &[(&str, &str)] = &[(
    "0001",
    "CREATE TABLE memory_entries(
        id TEXT PRIMARY KEY,
        body TEXT NOT NULL,
        approved INTEGER NOT NULL,
        created_at INTEGER NOT NULL
    ) WITHOUT ROWID;",
)];

fn storage(e: impl std::fmt::Display) -> MemoryError {
    MemoryError::storage(e)
}

/// Operacje na wpisach jednej sesji.
pub struct Store<'a> {
    db: Arc<Db>,
    indexer: &'a dyn TxIndexer,
    session: SessionId,
}

fn prepare(conn: &Connection, indexer: &dyn TxIndexer) -> Result<(), MemoryError> {
    migrate(conn, NAMESPACE, MIGRATIONS).map_err(storage)?;
    indexer.prepare(conn).map_err(storage)
}

fn doc_id(id: &MemoryId) -> DocId {
    DocId::new(DocKind::Memory, id.0.clone())
}

fn load_in(conn: &Connection, id: &MemoryId) -> Result<Option<MemoryEntry>, MemoryError> {
    let body: Option<String> = conn
        .query_row(
            "SELECT body FROM memory_entries WHERE id = ?1",
            params![id.0],
            |r| r.get(0),
        )
        .optional()
        .map_err(storage)?;
    body.map(|b| serde_json::from_str(&b).map_err(storage))
        .transpose()
}

impl<'a> Store<'a> {
    /// Uchwyt wpisów sesji.
    pub fn new(db: Arc<Db>, indexer: &'a dyn TxIndexer, session: SessionId) -> Self {
        Self {
            db,
            indexer,
            session,
        }
    }

    /// Zapis wpisu i dokumentu indeksu (atomowo).
    pub fn insert(&self, entry: &MemoryEntry) -> Result<(), MemoryError> {
        let body = serde_json::to_string(entry).map_err(storage)?;
        self.db.with(|conn| {
            let tx = conn.transaction().map_err(storage)?;
            prepare(&tx, self.indexer)?;
            tx.execute(
                "INSERT INTO memory_entries(id, body, approved, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![entry.id.0, body, entry.approved, entry.created_at.timestamp_millis()],
            )
            .map_err(storage)?;
            let doc = Doc {
                id: doc_id(&entry.id),
                session: self.session.clone(),
                text: entry.text.clone(),
                ts: entry.created_at,
            };
            self.indexer.index_in(&tx, &doc).map_err(storage)?;
            tx.commit().map_err(storage)
        })
    }

    /// Wpis albo `None`.
    pub fn load(&self, id: &MemoryId) -> Result<Option<MemoryEntry>, MemoryError> {
        self.db.with(|conn| {
            prepare(conn, self.indexer)?;
            load_in(conn, id)
        })
    }

    /// Wszystkie wpisy rosnąco po utworzeniu.
    pub fn list(&self) -> Result<Vec<MemoryEntry>, MemoryError> {
        self.db.with(|conn| {
            prepare(conn, self.indexer)?;
            let mut stmt = conn
                .prepare("SELECT body FROM memory_entries ORDER BY created_at, id")
                .map_err(storage)?;
            let bodies = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage)?;
            bodies
                .iter()
                .map(|b| serde_json::from_str(b).map_err(storage))
                .collect()
        })
    }

    /// Zatwierdza wpis oczekujący.
    pub fn approve(&self, id: &MemoryId) -> Result<MemoryEntry, MemoryError> {
        self.db.with(|conn| {
            prepare(conn, self.indexer)?;
            let mut entry =
                load_in(conn, id)?.ok_or_else(|| MemoryError::NotFound { id: id.clone() })?;
            entry.approved = true;
            let body = serde_json::to_string(&entry).map_err(storage)?;
            conn.execute(
                "UPDATE memory_entries SET body = ?2, approved = 1 WHERE id = ?1",
                params![id.0, body],
            )
            .map_err(storage)?;
            Ok(entry)
        })
    }

    /// Kaskada: wpis + FTS + wektor w jednej transakcji.
    pub fn forget(&self, id: &MemoryId) -> Result<ForgetReport, MemoryError> {
        self.db.with(|conn| {
            let tx = conn.transaction().map_err(storage)?;
            prepare(&tx, self.indexer)?;
            let removed = tx
                .execute("DELETE FROM memory_entries WHERE id = ?1", params![id.0])
                .map_err(storage)?;
            if removed == 0 {
                return Err(MemoryError::NotFound { id: id.clone() });
            }
            let index = self
                .indexer
                .remove_in(&tx, &self.session, &doc_id(id))
                .map_err(storage)?;
            tx.commit().map_err(storage)?;
            Ok(ForgetReport {
                entry: true,
                fts_rows: index.fts_rows,
                vectors: index.vectors,
                derived: 0,
            })
        })
    }
}
