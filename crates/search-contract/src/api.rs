//! Traity: usługa wyszukiwania, indeksowanie w transakcji właściciela bazy, embedder.

use lib_sqlstore::Db;
use lib_sqlstore::rusqlite::Connection;

use crate::error::SearchError;
use crate::types::{Caller, ConnQuery, Doc, DocId, Hit, Query, RemoveReport};
use crate::vectors::{ReindexProgress, VectorStatus};
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

    /// Zaciera dane usuniętych dokumentów w indeksie (FTS5 `optimize` scala segmenty, więc słowa
    /// usuniętych dokumentów znikają także z wewnętrznych tabel indeksu). Wywoływane po kaskadzie
    /// `forget` poza transakcją. Domyślnie nic nie robi.
    fn compact_in(&self, conn: &Connection) -> Result<(), SearchError> {
        let _ = conn;
        Ok(())
    }

    /// Stan wektorów w bazie (gotowe / przebudowa po zmianie embeddera / brakujące wektory).
    /// Domyślnie: gotowe.
    fn vector_status_in(&self, conn: &Connection) -> Result<VectorStatus, SearchError> {
        let _ = conn;
        Ok(VectorStatus::default())
    }

    /// Jeden krok przebudowy wektorów bazy (≤ `batch` dokumentów): odczyt i zapis w krótkich,
    /// osobnych blokadach połączenia, embedding **poza blokadą** (FTS i zapisy nie czekają na model).
    /// Postęp jest trwały (kursor w bazie) — przerwana przebudowa wznawia się od miejsca przerwania.
    /// Wywołujący powtarza kroki do `finished`. Domyślnie: nic do zrobienia.
    fn reindex_step(&self, db: &Db, batch: usize) -> Result<ReindexProgress, SearchError> {
        let _ = (db, batch);
        Ok(ReindexProgress::finished(0))
    }
}

/// Zapytanie **w połączeniu modułu, który jest właścicielem bazy** (np. `memory` w bazie zakresu
/// projektu/agentki/globalnego albo w bazie sesji) — bez [`crate::authorize`]: moduł wywołujący
/// sam sprawdził uprawnienia do bazy. `label` trafia do [`Hit::session`] (etykieta bazy).
pub trait TxSearcher: Send + Sync {
    /// Trafienia w jednej bazie, deterministycznie posortowane (co najwyżej `query.limit`).
    fn query_in(
        &self,
        conn: &Connection,
        label: &SessionId,
        query: &ConnQuery,
    ) -> Result<Vec<Hit>, SearchError>;
}

/// Lokalny embedder tekstu (ONNX w F7: `lib-embed`; w testach deterministyczna atrapa z `search-fake`).
pub trait Embedder: Send + Sync {
    /// Identyfikator modelu (zapisywany w bazie; zmiana → przebudowa wektorów w tle).
    fn model_id(&self) -> &str;
    /// Wymiar wektora.
    fn dims(&self) -> usize;
    /// Embeddingi **dokumentów** (po jednym wektorze `dims()` na tekst, znormalizowane L2).
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError>;
    /// Embeddingi **zapytań** — modele asymetryczne (E5: prefiks `query: ` zamiast `passage: `).
    /// Domyślnie jak [`Embedder::embed`].
    fn embed_query(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        self.embed(texts)
    }
}
