//! Pomocnicze dla testów przebudowy (`tests/reindex.rs`, `tests/reindex_props.rs`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use lib_sqlstore::Db;
use lib_sqlstore::rusqlite::Connection;
use search_contract::contract_tests::doc;
use search_contract::{DocKind, Embedder, Mode, Query, Search, SearchError, SessionId};
use search_fake::HashEmbedder;
use search_impl::SqliteSearch;
use sessions_contract::SessionDbProvider;

/// Embedder 8-wymiarowy: wektor `HashEmbedder` zwinięty do 8 współrzędnych; opcjonalna awaria
/// i jednorazowy „hak” wywoływany w trakcie embeddingu wsadu (zmiana dokumentu w międzyczasie).
#[derive(Default)]
pub struct Other {
    pub fail: AtomicBool,
    /// Wsad z tekstem zawierającym „trucizna” zawodzi (np. panika modelu na jednym wejściu).
    pub poison: AtomicBool,
    pub hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl Other {
    pub fn vector(text: &str) -> Vec<f32> {
        let mut v = [0.0_f32; 8];
        for (i, x) in HashEmbedder::vector(text).iter().enumerate() {
            v[i % 8] += x;
        }
        let n = v
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt()
            .max(f32::EPSILON);
        v.iter().map(|x| x / n).collect()
    }
}

impl Embedder for Other {
    fn model_id(&self) -> &str {
        "inny"
    }
    fn dims(&self) -> usize {
        8
    }
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        let poisoned =
            self.poison.load(Ordering::SeqCst) && texts.iter().any(|t| t.contains("trucizna"));
        if self.fail.load(Ordering::SeqCst) || poisoned {
            return Err(SearchError::Embedder {
                reason: "model niedostępny".into(),
            });
        }
        if texts.len() > 1
            && let Some(hook) = self.hook.lock().unwrap().take()
        {
            hook();
        }
        Ok(texts.iter().map(|t| Other::vector(t)).collect())
    }
}

pub fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// Tabele `vec0` (wirtualne; bez tabel pomocniczych `_chunks`, `_rowids`…).
pub fn tables(conn: &Connection) -> Vec<String> {
    conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'search_vec_%'
         AND sql LIKE 'CREATE VIRTUAL TABLE%' ORDER BY name",
    )
    .unwrap()
    .query_map([], |r| r.get(0))
    .unwrap()
    .collect::<Result<Vec<String>, _>>()
    .unwrap()
}

pub fn seeded(n: usize) -> (super::Harness, Arc<Db>) {
    let h = super::harness();
    for i in 0..n {
        let text = format!("dokument {i} o jeziorze i łodzi numer {i}");
        h.index(&doc("A", DocKind::Turn, &i.to_string(), &text))
            .unwrap();
    }
    h.index(&doc(
        "A",
        DocKind::Memory,
        "m1",
        "Karolina lubi zielony kolor",
    ))
    .unwrap();
    let db = h.provider.session_db(&SessionId::new("A")).unwrap();
    (h, db)
}

pub fn other(h: &super::Harness, embedder: Arc<Other>) -> Arc<SqliteSearch> {
    Arc::new(SqliteSearch::new(h.provider.clone(), embedder).unwrap())
}

pub fn q(text: &str, mode: Mode) -> Query {
    Query {
        mode,
        ..Query::in_session(SessionId::new("A"), text, 5)
    }
}
