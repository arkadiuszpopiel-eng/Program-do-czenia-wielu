//! Późne wiązanie `SessionDbProvider` i `TxIndexer`: `sessions` potrzebuje indeksera (`search`) przy budowie,
//! a `search` potrzebuje bazy sesji — cykl rozcina pośrednik ustawiany po zbudowaniu sesji.

use std::sync::{Arc, OnceLock, Weak};

use lib_sqlstore::Db;
use lib_sqlstore::rusqlite::Connection;
use search_contract::{
    Doc, DocId, ReindexProgress, RemoveReport, SearchError, TxIndexer, VectorStatus,
};
use sessions_contract::{SessionDbProvider, SessionError, SessionId};

/// Pośrednik do `SessionDbProvider` ustawianego raz, po zbudowaniu modułu sesji.
#[derive(Default)]
pub struct LateDbProvider {
    target: OnceLock<Weak<dyn SessionDbProvider>>,
}

impl LateDbProvider {
    /// Ustawia docelowego dostawcę (słaba referencja — bez cyklu `Arc`).
    pub fn bind(&self, target: &Arc<dyn SessionDbProvider>) {
        let _ = self.target.set(Arc::downgrade(target));
    }

    fn target(&self) -> Result<Arc<dyn SessionDbProvider>, SessionError> {
        self.target
            .get()
            .and_then(Weak::upgrade)
            .ok_or_else(|| SessionError::storage("moduł sesji nie jest jeszcze gotowy"))
    }
}

impl SessionDbProvider for LateDbProvider {
    fn session_db(&self, id: &SessionId) -> Result<Arc<Db>, SessionError> {
        self.target()?.session_db(id)
    }

    fn session_ids(&self) -> Result<Vec<SessionId>, SessionError> {
        self.target()?.session_ids()
    }
}

/// Pośrednik indeksera (`search`) dla `sessions` — wiązany zaraz po zbudowaniu wyszukiwania,
/// przed pierwszym zapisem tury. Niezwiązany indekser zwraca błąd (tura nie zapisze się bez indeksu).
#[derive(Default)]
pub struct LateIndexer {
    target: OnceLock<Weak<dyn TxIndexer>>,
}

impl LateIndexer {
    /// Ustawia indekser.
    pub fn bind(&self, target: &Arc<dyn TxIndexer>) {
        let _ = self.target.set(Arc::downgrade(target));
    }

    fn target(&self) -> Result<Arc<dyn TxIndexer>, SearchError> {
        self.target
            .get()
            .and_then(Weak::upgrade)
            .ok_or_else(|| SearchError::storage("indeks wyszukiwania nie jest jeszcze gotowy"))
    }
}

impl TxIndexer for LateIndexer {
    fn prepare(&self, conn: &Connection) -> Result<(), SearchError> {
        self.target()?.prepare(conn)
    }

    fn index_in(&self, conn: &Connection, doc: &Doc) -> Result<(), SearchError> {
        self.target()?.index_in(conn, doc)
    }

    fn remove_in(
        &self,
        conn: &Connection,
        session: &SessionId,
        id: &DocId,
    ) -> Result<RemoveReport, SearchError> {
        self.target()?.remove_in(conn, session, id)
    }

    // Metody z domyślną implementacją w kontrakcie też muszą trafić do `search` — inaczej sesje
    // dostałyby „nic do zrobienia” (bez kompakcji FTS i bez przebudowy wektorów po zmianie modelu).
    fn compact_in(&self, conn: &Connection) -> Result<(), SearchError> {
        self.target()?.compact_in(conn)
    }

    fn vector_status_in(&self, conn: &Connection) -> Result<VectorStatus, SearchError> {
        self.target()?.vector_status_in(conn)
    }

    fn reindex_step(&self, db: &Db, batch: usize) -> Result<ReindexProgress, SearchError> {
        self.target()?.reindex_step(db, batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Indekser-sonda: wyniki rozpoznawalne po przekazaniu przez pośrednika.
    struct Probe;

    impl TxIndexer for Probe {
        fn prepare(&self, _: &Connection) -> Result<(), SearchError> {
            Ok(())
        }
        fn index_in(&self, _: &Connection, _: &Doc) -> Result<(), SearchError> {
            Ok(())
        }
        fn remove_in(
            &self,
            _: &Connection,
            _: &SessionId,
            _: &DocId,
        ) -> Result<RemoveReport, SearchError> {
            Ok(RemoveReport::default())
        }
        fn compact_in(&self, _: &Connection) -> Result<(), SearchError> {
            Err(SearchError::storage("kompakcja dotarła"))
        }
        fn vector_status_in(&self, _: &Connection) -> Result<VectorStatus, SearchError> {
            Ok(VectorStatus::Rebuilding {
                from: "a/1".into(),
                to: "b/2".into(),
                done: 1,
                total: 2,
            })
        }
        fn reindex_step(&self, _: &Db, batch: usize) -> Result<ReindexProgress, SearchError> {
            Ok(ReindexProgress {
                embedded: batch as u64,
                done: 0,
                total: 0,
                finished: false,
            })
        }
    }

    #[test]
    fn bound_indexer_forwards_compaction_and_vector_rebuild() {
        let target: Arc<dyn TxIndexer> = Arc::new(Probe);
        let late = LateIndexer::default();
        late.bind(&target);
        let path = std::env::temp_dir().join(format!("alfa-late-{}.db", std::process::id()));
        let db = Db::open(&path, &lib_sqlstore::DbKey::from_bytes([7; 32])).unwrap();
        db.with(|c| {
            assert!(late.compact_in(c).is_err());
            assert!(matches!(
                late.vector_status_in(c),
                Ok(VectorStatus::Rebuilding { done: 1, .. })
            ));
            Ok::<(), SearchError>(())
        })
        .unwrap();
        assert_eq!(late.reindex_step(&db, 5).unwrap().embedded, 5);
        drop(db);
        let _ = lib_sqlstore::remove_database(&path);
    }

    #[test]
    fn unbound_provider_reports_storage_error() {
        let late = LateDbProvider::default();
        assert!(matches!(
            late.session_ids(),
            Err(SessionError::Storage { .. })
        ));
    }
}
