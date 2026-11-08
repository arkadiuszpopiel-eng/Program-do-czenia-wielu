//! Stan wektorów bazy: generacje tabel `vec0` per embedder i przebudowa po zmianie embeddera.
//!
//! `search_meta`: `embedder` (aktywny, `model_id/wymiar`), `vec_gen` (aktywna generacja; brak = 0),
//! podczas przebudowy `target` (bieżący embedder), `target_gen`, `cursor` (ostatni przeliczony
//! `search_docs.id`). Generacja 0 = historyczne nazwy `search_vec_<rodzaj>`, kolejne
//! `search_vec_<rodzaj>_g<N>`. `search_vec_missing(id, gen)` = dokument bez wektora w generacji
//! (embedder niedostępny przy zapisie albo stara generacja po zapisie w trakcie przebudowy).
//!
//! Przebudowa: wykrycie innego embeddera → nowe tabele docelowe, kursor 0; zapisy trafiają do
//! generacji docelowej; zapytania wektorowe spadają do FTS; krok przebudowy przelicza dokumenty
//! powyżej kursora; koniec kursora → usunięcie starych tabel i przełączenie generacji (atomowo).
//! Powrót do poprzedniego embeddera w trakcie → porzucenie generacji docelowej.

use std::collections::BTreeMap;

use lib_sqlstore::migrate;
use lib_sqlstore::rusqlite::{Connection, params};
use search_contract::{DocKind, Embedder, SearchError};

/// Przestrzeń nazw migracji.
pub const NAMESPACE: &str = "search";

/// Migracje stałe: dokumenty (oryginalny tekst do fragmentów), FTS5 na kolumnie złożonej
/// `fold_pl` (tokenizer `unicode61`, diakrytyki usuwane także dla pisma innego niż łacińskie),
/// metadane indeksu (embedder); `0002` — brakujące wektory per generacja. Tabele wektorowe zależą
/// od wymiaru embeddera — [`prepare`].
pub const MIGRATIONS: &[(&str, &str)] = &[
    (
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
    ),
    (
        "0002",
        "CREATE TABLE search_vec_missing(id INTEGER NOT NULL, gen INTEGER NOT NULL,
        PRIMARY KEY(id, gen)) WITHOUT ROWID;",
    ),
];

pub fn storage(e: impl std::fmt::Display) -> SearchError {
    SearchError::storage(e)
}

/// Nazwa tabeli `vec0` generacji 0 dla rodzaju dokumentu (osobna na rodzaj, żeby kNN z filtrem
/// rodzaju nie gubił rzadkich dokumentów, np. kilku wpisów pamięci wśród tysięcy tur).
pub fn vec_table(kind: DocKind) -> String {
    vec_table_gen(kind, 0)
}

/// Nazwa tabeli `vec0` generacji `generation`.
pub fn vec_table_gen(kind: DocKind, generation: i64) -> String {
    if generation == 0 {
        format!("search_vec_{}", kind.as_str())
    } else {
        format!("search_vec_{}_g{generation}", kind.as_str())
    }
}

/// Stan wektorów bazy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VecState {
    /// Aktywna generacja (zapytania).
    pub active_gen: i64,
    /// Embedder aktywnej generacji.
    pub active: String,
    /// Przebudowa: (generacja docelowa, embedder docelowy = bieżący, kursor).
    pub target: Option<(i64, String, i64)>,
    /// Czy przebudowa zaczęła się w tym wywołaniu (zdarzenie `search.reindex.started`).
    pub just_started: bool,
}

impl VecState {
    /// Czy zapytania mogą korzystać z wektorów.
    pub fn usable(&self) -> bool {
        self.target.is_none()
    }

    /// Generacja, do której trafiają nowe wektory.
    pub fn write_gen(&self) -> i64 {
        self.target.as_ref().map_or(self.active_gen, |t| t.0)
    }

    /// Ten sam stan bez flagi startu (porównanie faz kroku przebudowy).
    pub fn same(&self, other: &VecState) -> bool {
        (self.active_gen, &self.active, &self.target)
            == (other.active_gen, &other.active, &other.target)
    }
}

/// Identyfikator embeddera w bazie.
pub fn embedder_key(embedder: &dyn Embedder) -> String {
    format!("{}/{}", embedder.model_id(), embedder.dims())
}

fn read_meta(conn: &Connection) -> Result<BTreeMap<String, String>, SearchError> {
    let mut stmt = conn
        .prepare_cached("SELECT key, value FROM search_meta")
        .map_err(storage)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(storage)?;
    rows.collect::<Result<_, _>>().map_err(storage)
}

fn int(meta: &BTreeMap<String, String>, key: &str) -> i64 {
    meta.get(key).and_then(|v| v.parse().ok()).unwrap_or(0)
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), SearchError> {
    conn.execute(
        "INSERT INTO search_meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(storage)?;
    Ok(())
}

fn del_meta(conn: &Connection, keys: &[&str]) -> Result<(), SearchError> {
    for key in keys {
        conn.execute("DELETE FROM search_meta WHERE key = ?1", params![key])
            .map_err(storage)?;
    }
    Ok(())
}

fn create_tables(conn: &Connection, generation: i64, dims: usize) -> Result<(), SearchError> {
    let mut sql = String::from("SAVEPOINT search_vec;");
    for kind in DocKind::ALL {
        sql.push_str(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {} USING vec0(embedding float[{dims}] distance_metric=cosine, chunk_size=128);",
            vec_table_gen(kind, generation),
        ));
    }
    sql.push_str("RELEASE search_vec;");
    conn.execute_batch(&sql).map_err(storage)
}

/// Usuwa tabele generacji i jej wpisy brakujących wektorów.
pub fn drop_generation(conn: &Connection, generation: i64) -> Result<(), SearchError> {
    let mut sql = String::from("SAVEPOINT search_drop;");
    for kind in DocKind::ALL {
        sql.push_str(&format!(
            "DROP TABLE IF EXISTS {};",
            vec_table_gen(kind, generation)
        ));
    }
    sql.push_str(&format!(
        "DELETE FROM search_vec_missing WHERE gen = {generation};RELEASE search_drop;"
    ));
    conn.execute_batch(&sql).map_err(storage)
}

/// Migruje schemat i ustala stan wektorów dla bieżącego embeddera (tworzy tabele pierwszej
/// generacji, zaczyna/wznawia/porzuca przebudowę). Idempotentne.
pub fn prepare(conn: &Connection, embedder: &dyn Embedder) -> Result<VecState, SearchError> {
    migrate(conn, NAMESPACE, MIGRATIONS).map_err(storage)?;
    let current = embedder_key(embedder);
    let meta = read_meta(conn)?;
    let active_gen = int(&meta, "vec_gen");
    let target_gen = int(&meta, "target_gen");
    let Some(active) = meta.get("embedder").cloned() else {
        create_tables(conn, 0, embedder.dims())?;
        set_meta(conn, "embedder", &current)?;
        return Ok(VecState {
            active_gen: 0,
            active: current,
            target: None,
            just_started: false,
        });
    };
    let target = meta.get("target");
    if active == current {
        if target.is_some() {
            drop_generation(conn, target_gen)?;
            del_meta(conn, &["target", "target_gen", "cursor"])?;
        }
        return Ok(VecState {
            active_gen,
            active,
            target: None,
            just_started: false,
        });
    }
    if target == Some(&current) {
        return Ok(VecState {
            active_gen,
            active,
            target: Some((target_gen, current, int(&meta, "cursor"))),
            just_started: false,
        });
    }
    if target.is_some() {
        drop_generation(conn, target_gen)?;
    }
    let generation = active_gen.max(target_gen) + 1;
    create_tables(conn, generation, embedder.dims())?;
    set_meta(conn, "target", &current)?;
    set_meta(conn, "target_gen", &generation.to_string())?;
    set_meta(conn, "cursor", "0")?;
    Ok(VecState {
        active_gen,
        active,
        target: Some((generation, current, 0)),
        just_started: true,
    })
}

/// Kończy przebudowę: generacja docelowa staje się aktywną, stare tabele są usuwane.
pub fn finish_rebuild(conn: &Connection, state: &VecState) -> Result<(), SearchError> {
    let Some((generation, embedder, _)) = &state.target else {
        return Ok(());
    };
    drop_generation(conn, state.active_gen)?;
    conn.execute(
        "DELETE FROM search_vec_missing WHERE gen <> ?1",
        params![generation],
    )
    .map_err(storage)?;
    set_meta(conn, "embedder", embedder)?;
    set_meta(conn, "vec_gen", &generation.to_string())?;
    del_meta(conn, &["target", "target_gen", "cursor"])
}
