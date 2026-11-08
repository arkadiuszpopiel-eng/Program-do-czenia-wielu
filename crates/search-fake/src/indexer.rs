//! `TxIndexer` rejestrujący wywołania (bez bazy) — do testów modułów zapisujących dane.

use std::sync::{Mutex, PoisonError};

use lib_sqlstore::rusqlite::Connection;
use search_contract::{Doc, DocId, RemoveReport, SearchError, SessionId, TxIndexer};

/// Zapisuje dokumenty przekazane do `index_in`/`remove_in`; opcjonalnie zwraca błąd (test wycofania
/// transakcji po stronie wywołującego).
#[derive(Debug, Default)]
pub struct RecordingIndexer {
    indexed: Mutex<Vec<Doc>>,
    removed: Mutex<Vec<DocId>>,
    fail: Mutex<bool>,
}

impl RecordingIndexer {
    /// Nowy indekser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dokumenty przekazane do `index_in` (w kolejności).
    pub fn indexed(&self) -> Vec<Doc> {
        self.indexed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Identyfikatory przekazane do `remove_in`.
    pub fn removed(&self) -> Vec<DocId> {
        self.removed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Kolejne `index_in` zwrócą błąd magazynu.
    pub fn set_failing(&self, failing: bool) {
        *self.fail.lock().unwrap_or_else(PoisonError::into_inner) = failing;
    }
}

impl TxIndexer for RecordingIndexer {
    fn prepare(&self, _conn: &Connection) -> Result<(), SearchError> {
        Ok(())
    }

    fn index_in(&self, _conn: &Connection, doc: &Doc) -> Result<(), SearchError> {
        if *self.fail.lock().unwrap_or_else(PoisonError::into_inner) {
            return Err(SearchError::storage("atrapa: indeksowanie wyłączone"));
        }
        self.indexed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(doc.clone());
        Ok(())
    }

    fn remove_in(
        &self,
        _conn: &Connection,
        _session: &SessionId,
        id: &DocId,
    ) -> Result<RemoveReport, SearchError> {
        self.removed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(id.clone());
        Ok(RemoveReport::default())
    }
}
