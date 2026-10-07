//! Zapis i usuwanie dokumentów (w transakcji wywołującego): wiersz, FTS i wektor w generacji
//! zapisu ([`VecState::write_gen`]). Embedder niedostępny → dokument trafia do FTS, a brak wektora
//! jest zapisany w `search_vec_missing` (uzupełni krok przebudowy) — zapis danych się nie wycofuje.

use lib_sqlstore::rusqlite::{Connection, OptionalExtension, params};
use lib_sqlstore::{fold_pl, vector_to_blob};
use search_contract::{Doc, DocId, DocKind, Embedder, RemoveReport, SearchError};

use crate::state::VecState;
pub use crate::state::{storage, vec_table_gen};

/// Sprawdza wymiar wektorów zwróconych przez embedder.
pub fn check_vectors(
    embedder: &dyn Embedder,
    vectors: Vec<Vec<f32>>,
    expected: usize,
) -> Result<Vec<Vec<f32>>, SearchError> {
    if vectors.len() != expected {
        return Err(SearchError::Embedder {
            reason: format!("{} wektorów ≠ {expected} tekstów", vectors.len()),
        });
    }
    if let Some(bad) = vectors.iter().find(|v| v.len() != embedder.dims()) {
        return Err(SearchError::Embedder {
            reason: format!("wymiar {} ≠ {}", bad.len(), embedder.dims()),
        });
    }
    Ok(vectors)
}

/// Embedding jednego dokumentu z kontrolą wymiaru.
pub fn embed_one(embedder: &dyn Embedder, text: &str) -> Result<Vec<f32>, SearchError> {
    let mut v = check_vectors(embedder, embedder.embed(&[text])?, 1)?;
    v.pop().ok_or_else(|| SearchError::Embedder {
        reason: "brak wektora".into(),
    })
}

/// Embedding jednego zapytania z kontrolą wymiaru.
pub fn embed_query_one(embedder: &dyn Embedder, text: &str) -> Result<Vec<f32>, SearchError> {
    let mut v = check_vectors(embedder, embedder.embed_query(&[text])?, 1)?;
    v.pop().ok_or_else(|| SearchError::Embedder {
        reason: "brak wektora".into(),
    })
}

fn find(conn: &Connection, id: &DocId) -> Result<Option<i64>, SearchError> {
    conn.query_row(
        "SELECT id FROM search_docs WHERE kind = ?1 AND key = ?2",
        params![id.kind.as_str(), id.key],
        |r| r.get(0),
    )
    .optional()
    .map_err(storage)
}

/// Wstawia wektor dokumentu w generacji (zastępuje istniejący).
pub fn put_vector(
    conn: &Connection,
    kind: DocKind,
    generation: i64,
    rowid: i64,
    vector: &[f32],
) -> Result<(), SearchError> {
    let table = vec_table_gen(kind, generation);
    conn.execute(
        &format!("DELETE FROM {table} WHERE rowid = ?1"),
        params![rowid],
    )
    .map_err(storage)?;
    conn.execute(
        &format!("INSERT INTO {table}(rowid, embedding) VALUES (?1, ?2)"),
        params![rowid, vector_to_blob(vector)],
    )
    .map_err(storage)?;
    conn.execute(
        "DELETE FROM search_vec_missing WHERE id = ?1 AND gen = ?2",
        params![rowid, generation],
    )
    .map_err(storage)?;
    Ok(())
}

/// Zapisuje brak wektora dokumentu w generacji.
pub fn mark_missing(conn: &Connection, rowid: i64, generation: i64) -> Result<(), SearchError> {
    conn.execute(
        "INSERT OR IGNORE INTO search_vec_missing(id, gen) VALUES (?1, ?2)",
        params![rowid, generation],
    )
    .map_err(storage)?;
    Ok(())
}

/// Zapisuje dokument (zastępuje istniejący o tym samym `DocId`): wiersz, FTS i wektor. Zwraca
/// `false`, gdy wektora nie udało się policzyć (dokument jest w FTS, brak w `search_vec_missing`).
pub fn index_doc(
    conn: &Connection,
    embedder: &dyn Embedder,
    state: &VecState,
    doc: &Doc,
) -> Result<bool, SearchError> {
    let ts = doc.ts.timestamp_millis();
    let rowid = match find(conn, &doc.id)? {
        Some(rowid) => {
            delete_index_rows(conn, state, doc.id.kind, rowid)?;
            conn.execute(
                "UPDATE search_docs SET text = ?2, ts = ?3 WHERE id = ?1",
                params![rowid, doc.text, ts],
            )
            .map_err(storage)?;
            rowid
        }
        None => {
            conn.execute(
                "INSERT INTO search_docs(kind, key, text, ts) VALUES (?1, ?2, ?3, ?4)",
                params![doc.id.kind.as_str(), doc.id.key, doc.text, ts],
            )
            .map_err(storage)?;
            conn.last_insert_rowid()
        }
    };
    conn.execute(
        "INSERT INTO search_fts(rowid, folded) VALUES (?1, ?2)",
        params![rowid, fold_pl(&doc.text)],
    )
    .map_err(storage)?;
    if state.target.is_some() {
        // Stara generacja nie ma wektora tego tekstu (ważne, gdy przebudowa zostanie porzucona).
        mark_missing(conn, rowid, state.active_gen)?;
    }
    let generation = state.write_gen();
    match embed_one(embedder, &doc.text) {
        Ok(vector) => {
            put_vector(conn, doc.id.kind, generation, rowid, &vector)?;
            Ok(true)
        }
        Err(SearchError::Embedder { .. }) => {
            mark_missing(conn, rowid, generation)?;
            Ok(false)
        }
        Err(other) => Err(other),
    }
}

fn delete_index_rows(
    conn: &Connection,
    state: &VecState,
    kind: DocKind,
    rowid: i64,
) -> Result<(usize, usize), SearchError> {
    let fts = conn
        .execute("DELETE FROM search_fts WHERE rowid = ?1", params![rowid])
        .map_err(storage)?;
    let mut vec = 0;
    let generations = std::iter::once(state.active_gen).chain(state.target.as_ref().map(|t| t.0));
    for generation in generations {
        vec += conn
            .execute(
                &format!(
                    "DELETE FROM {} WHERE rowid = ?1",
                    vec_table_gen(kind, generation)
                ),
                params![rowid],
            )
            .map_err(storage)?;
    }
    conn.execute(
        "DELETE FROM search_vec_missing WHERE id = ?1",
        params![rowid],
    )
    .map_err(storage)?;
    Ok((fts, vec))
}

/// Usuwa dokument z wiersza, FTS i wektorów wszystkich generacji (kaskada `forget`/usunięcia).
pub fn remove_doc(
    conn: &Connection,
    state: &VecState,
    id: &DocId,
) -> Result<RemoveReport, SearchError> {
    let Some(rowid) = find(conn, id)? else {
        return Ok(RemoveReport::default());
    };
    let (fts_rows, vectors) = delete_index_rows(conn, state, id.kind, rowid)?;
    let docs = conn
        .execute("DELETE FROM search_docs WHERE id = ?1", params![rowid])
        .map_err(storage)?;
    Ok(RemoveReport {
        docs,
        fts_rows,
        vectors,
    })
}

/// Zatarcie usuniętych danych indeksu: FTS5 `optimize` scala segmenty (znikają wpisy usuniętych
/// dokumentów z `search_fts_data`); `vec0` zeruje wektor przy usunięciu sam.
pub fn compact(conn: &Connection) -> Result<(), SearchError> {
    conn.execute("INSERT INTO search_fts(search_fts) VALUES ('optimize')", [])
        .map_err(storage)?;
    Ok(())
}
