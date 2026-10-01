//! Późne wiązanie `SessionDbProvider` i `TxIndexer`: `sessions` potrzebuje indeksera (`search`) przy budowie,
//! a `search` potrzebuje bazy sesji — cykl rozcina pośrednik ustawiany po zbudowaniu sesji.

use std::sync::{Arc, OnceLock, Weak};

use lib_sqlstore::Db;
use lib_sqlstore::rusqlite::Connection;
use search_contract::{Doc, DocId, RemoveReport, SearchError, TxIndexer};
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbound_provider_reports_storage_error() {
        let late = LateDbProvider::default();
        assert!(matches!(
            late.session_ids(),
            Err(SessionError::Storage { .. })
        ));
    }
}
