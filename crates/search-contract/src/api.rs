//! Traity: usługa wyszukiwania, indeksowanie w transakcji właściciela bazy, embedder.

use lib_sqlstore::rusqlite::Connection;

use crate::error::SearchError;
use crate::types::{Caller, Doc, DocId, Hit, Query, RemoveReport};
use core_bus_contract::SessionId;

/// Usługa wyszukiwania (UI, pamięć, narzędzia agentek w obrębie własnej sesji).
pub trait Search: Send + Sync {
    /// Indeksuje dokument w bazie jego sesji (ten sam `DocId` → zastąpienie).
    fn index(&self, doc: &Doc) -> Result<(), SearchError>;
    /// Usuwa dokument z indeksu (FTS + wektor); kaskada `forget`/usunięcia.
    fn remove(&self, session: &SessionId, id: &DocId) -> Result<RemoveReport, SearchError>;
    /// Zapytanie; najpierw [`crate::authorize`] (agentka tylko we własnej sesji).
    fn query(&self, query: &Query, caller: &Caller) -> Result<Vec<Hit>, SearchError>;
}

/// Indeksowanie **w transakcji modułu, który zapisuje dane** (np. `sessions` przy `append_turn`,
/// `memory` przy `remember`/`forget`): zapis danych i indeksu jest atomowy.
///
/// Implementacje nie mogą sięgać po bazę innymi drogami (np. przez `SessionDbProvider`) — połączenie
/// jest już zablokowane przez wywołującego.
pub trait TxIndexer: Send + Sync {
    /// Tworzy/migruje tabele indeksu w bazie (idempotentne; wywoływane po otwarciu bazy).
    fn prepare(&self, conn: &Connection) -> Result<(), SearchError>;
    /// Indeksuje dokument w podanym połączeniu (zastępuje istniejący o tym samym `DocId`).
    fn index_in(&self, conn: &Connection, doc: &Doc) -> Result<(), SearchError>;
    /// Usuwa dokument z indeksu w podanym połączeniu (`session` = sesja, do której należy baza).
    fn remove_in(
        &self,
        conn: &Connection,
        session: &SessionId,
        id: &DocId,
    ) -> Result<RemoveReport, SearchError>;
}

/// Lokalny embedder tekstu (ONNX w F7; w testach deterministyczna atrapa z `search-fake`).
pub trait Embedder: Send + Sync {
    /// Identyfikator modelu (zapisywany w bazie; zmiana wymaga reindeksacji).
    fn model_id(&self) -> &str;
    /// Wymiar wektora.
    fn dims(&self) -> usize;
    /// Embeddingi tekstów (po jednym wektorze `dims()` na tekst, znormalizowane L2).
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError>;
}
