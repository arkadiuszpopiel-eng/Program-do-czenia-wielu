//! Schemat indeksu w bazie sesji i zapis/usuwanie dokumentów (w transakcji wywołującego).

use lib_sqlstore::rusqlite::{Connection, OptionalExtension, params};
use lib_sqlstore::{fold_pl, migrate, vector_to_blob};
use search_contract::{Doc, DocId, DocKind, Embedder, RemoveReport, SearchError};

/// Przestrzeń nazw migracji.
pub const NAMESPACE: &str = "search";

/// Migracje stałe: dokumenty (oryginalny tekst do fragmentów), FTS5 na kolumnie złożonej
/// `fold_pl` (tokenizer `unicode61`, diakrytyki usuwane także dla pisma innego niż łacińskie),
/// metadane indeksu (embedder). Tabele wektorowe zależą od wymiaru embeddera — [`prepare`].
pub const MIGRATIONS: &[(&str, &str)] = &[(
    "0001",
    "CREATE TABLE search_docs(
        id INTEGER PRIMARY KEY,
        kind TEXT NOT NULL,
        key TEXT NOT NULL,
        text TEXT NOT NULL,
        ts INTEGER NOT NULL,
        UNIQUE(kind, key)
    );
    CREATE VIRTUAL TABLE search_fts USING fts5(folded, tokenize = 'unicode61 remove_diacritics 2');
    CREATE TABLE search_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;",
)];

pub fn storage(e: impl std::fmt::Display) -> SearchError {
    SearchError::storage(e)
}

/// Nazwa tabeli `vec0` dla rodzaju dokumentu (osobna na rodzaj, żeby kNN z filtrem rodzaju nie
/// gubił rzadkich dokumentów, np. kilku wpisów pamięci wśród tysięcy tur).
pub fn vec_table(kind: DocKind) -> String {
    format!("search_vec_{}", kind.as_str())
}

/// Migruje schemat i tworzy tabele wektorowe dla embeddera; inny embedder niż zapisany w bazie →
/// [`SearchError::EmbedderMismatch`] (wymagana reindeksacja).
pub fn prepare(conn: &Connection, embedder: &dyn Embedder) -> Result<(), SearchError> {
    migrate(conn, NAMESPACE, MIGRATIONS).map_err(storage)?;
    let current = format!("{}/{}", embedder.model_id(), embedder.dims());
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM search_meta WHERE key = 'embedder'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(storage)?;
    match stored {
        Some(indexed) if indexed == current => Ok(()),
        Some(indexed) => Err(SearchError::EmbedderMismatch { indexed, current }),
        None => {
            let mut sql = String::from("SAVEPOINT search_vec;");
            for kind in DocKind::ALL {
                sql.push_str(&format!(
                    "CREATE VIRTUAL TABLE IF NOT EXISTS {} USING vec0(embedding float[{}] distance_metric=cosine, chunk_size=128);",
                    vec_table(kind),
                    embedder.dims()
                ));
            }
            sql.push_str("RELEASE search_vec;");
            conn.execute_batch(&sql).map_err(storage)?;
            conn.execute(
                "INSERT INTO search_meta(key, value) VALUES ('embedder', ?1)",
                params![current],
            )
            .map_err(storage)?;
            Ok(())
        }
    }
}

/// Embedding jednego tekstu z kontrolą wymiaru.
pub fn embed_one(embedder: &dyn Embedder, text: &str) -> Result<Vec<f32>, SearchError> {
    let vector =
        embedder
            .embed(&[text])?
            .into_iter()
            .next()
            .ok_or_else(|| SearchError::Embedder {
                reason: "brak wektora".into(),
            })?;
    if vector.len() != embedder.dims() {
        return Err(SearchError::Embedder {
            reason: format!("wymiar {} ≠ {}", vector.len(), embedder.dims()),
        });
    }
    Ok(vector)
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

/// Zapisuje dokument (zastępuje istniejący o tym samym `DocId`): wiersz, FTS i wektor.
pub fn index_doc(conn: &Connection, embedder: &dyn Embedder, doc: &Doc) -> Result<(), SearchError> {
    let vector = embed_one(embedder, &doc.text)?;
    let ts = doc.ts.timestamp_millis();
    let rowid = match find(conn, &doc.id)? {
        Some(rowid) => {
            delete_index_rows(conn, doc.id.kind, rowid)?;
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
    conn.execute(
        &format!(
            "INSERT INTO {}(rowid, embedding) VALUES (?1, ?2)",
            vec_table(doc.id.kind)
        ),
        params![rowid, vector_to_blob(&vector)],
    )
    .map_err(storage)?;
    Ok(())
}

fn delete_index_rows(
    conn: &Connection,
    kind: DocKind,
    rowid: i64,
) -> Result<(usize, usize), SearchError> {
    let fts = conn
        .execute("DELETE FROM search_fts WHERE rowid = ?1", params![rowid])
        .map_err(storage)?;
    let vec = conn
        .execute(
            &format!("DELETE FROM {} WHERE rowid = ?1", vec_table(kind)),
            params![rowid],
        )
        .map_err(storage)?;
    Ok((fts, vec))
}

/// Usuwa dokument z wiersza, FTS i wektora (kaskada `forget`/usunięcia).
pub fn remove_doc(conn: &Connection, id: &DocId) -> Result<RemoveReport, SearchError> {
    let Some(rowid) = find(conn, id)? else {
        return Ok(RemoveReport::default());
    };
    let (fts_rows, vectors) = delete_index_rows(conn, id.kind, rowid)?;
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
