//! Atrapa modułu `search` (docs/modules/search/SPEC.md, „Fake”).
//!
//! - [`FakeSearch`] — indeks w pamięci: dopasowanie słów (bez diakrytyków, prefiksy, AND),
//!   kosinus na wektorach [`HashEmbedder`], hybryda RRF; te same reguły autoryzacji i fragmentów
//!   co `search-impl` (wspólne funkcje z `search-contract`).
//! - [`HashEmbedder`] — deterministyczny embedder (hash n-gramów → 64 wym.).
//! - [`RecordingIndexer`] — `TxIndexer`, który tylko zapisuje wywołania (testy `sessions`/`memory`).
//!
//! `FakeSearch` implementuje też `TxIndexer` (ignoruje połączenie, indeksuje w pamięci) — dzięki temu
//! moduł zapisujący dane (np. `memory-impl`) można testować z jedną spójną atrapą indeksu.
//! Uwaga: indeks atrapy nie jest wycofywany razem z transakcją wywołującego.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod embedder;
mod indexer;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard, PoisonError};

use lib_sqlstore::rusqlite::Connection;
use lib_sqlstore::{search_tokens, tokenize};
use search_contract::{
    Caller, DEFAULT_SNIPPET_CHARS, Doc, DocId, Hit, MAX_LIMIT, Mode, Query, RemoveReport, Search,
    SearchError, SessionId, SessionSet, TxIndexer, authorize, fuse_rrf, make_snippet, sort_hits,
};

pub use embedder::{HASH_DIMS, HashEmbedder, cosine};
pub use indexer::RecordingIndexer;

#[derive(Debug, Clone)]
struct Entry {
    text: String,
    terms: Vec<String>,
    vector: Vec<f32>,
}

/// Indeks w pamięci: `(sesja, dokument) → tekst + wektor`.
#[derive(Debug, Default)]
pub struct FakeSearch {
    docs: Mutex<BTreeMap<SessionId, BTreeMap<DocId, Entry>>>,
}

impl FakeSearch {
    /// Pusty indeks.
    pub fn new() -> Self {
        Self::default()
    }

    /// Liczba dokumentów w sesji.
    pub fn doc_count(&self, session: &SessionId) -> usize {
        self.lock().get(session).map_or(0, BTreeMap::len)
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<SessionId, BTreeMap<DocId, Entry>>> {
        self.docs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn query_session(
        docs: &BTreeMap<DocId, Entry>,
        session: &SessionId,
        q: &Query,
        limit: usize,
    ) -> Vec<Hit> {
        let terms = search_tokens(&q.text);
        let candidates = (limit * 4).max(20);
        let accepted = || docs.iter().filter(|(id, _)| q.accepts(id.kind));
        let mut fts: Vec<(DocId, f32)> = if terms.is_empty() {
            Vec::new()
        } else {
            accepted()
                .filter_map(|(id, e)| fts_score(&terms, &e.terms).map(|s| (id.clone(), s)))
                .collect()
        };
        fts.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        fts.truncate(candidates);
        let query_vec = HashEmbedder::vector(&q.text);
        let mut vec: Vec<(DocId, f32)> = accepted()
            .map(|(id, e)| (id.clone(), cosine(&query_vec, &e.vector)))
            .collect();
        vec.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        vec.truncate(candidates);
        let ranked: Vec<(DocId, f32)> = match q.mode {
            Mode::Fts => fts,
            Mode::Vector => vec,
            Mode::Hybrid => fuse_rrf(&[
                fts.into_iter().map(|(k, _)| k).collect(),
                vec.into_iter().map(|(k, _)| k).collect(),
            ]),
        };
        ranked
            .into_iter()
            .take(limit)
            .filter_map(|(id, score)| {
                docs.get(&id).map(|e| Hit {
                    snippet: make_snippet(&e.text, &q.text, DEFAULT_SNIPPET_CHARS),
                    doc: id,
                    session: session.clone(),
                    score,
                })
            })
            .collect()
    }
}

/// Wynik FTS atrapy: każde słowo zapytania musi być prefiksem jakiegoś słowa dokumentu (AND);
/// wynik = liczba pasujących wystąpień.
fn fts_score(query: &[String], doc: &[String]) -> Option<f32> {
    let mut total = 0_u16;
    for q in query {
        let n = doc.iter().filter(|t| t.starts_with(q.as_str())).count();
        if n == 0 {
            return None;
        }
        total = total.saturating_add(u16::try_from(n).unwrap_or(u16::MAX));
    }
    Some(f32::from(total))
}

impl Search for FakeSearch {
    fn index(&self, doc: &Doc) -> Result<(), SearchError> {
        let entry = Entry {
            terms: tokenize(&doc.text).into_iter().map(|t| t.term).collect(),
            vector: HashEmbedder::vector(&doc.text),
            text: doc.text.clone(),
        };
        self.lock()
            .entry(doc.session.clone())
            .or_default()
            .insert(doc.id.clone(), entry);
        Ok(())
    }

    fn remove(&self, session: &SessionId, id: &DocId) -> Result<RemoveReport, SearchError> {
        let removed = self
            .lock()
            .get_mut(session)
            .and_then(|docs| docs.remove(id))
            .is_some();
        let n = usize::from(removed);
        Ok(RemoveReport {
            docs: n,
            fts_rows: n,
            vectors: n,
        })
    }

    fn query(&self, query: &Query, caller: &Caller) -> Result<Vec<Hit>, SearchError> {
        authorize(&query.sessions, caller)?;
        let limit = query.limit.min(MAX_LIMIT);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let docs = self.lock();
        let sessions: BTreeSet<SessionId> = match &query.sessions {
            SessionSet::One(s) => BTreeSet::from([s.clone()]),
            SessionSet::Many(list) => list.iter().cloned().collect(),
            SessionSet::All => docs.keys().cloned().collect(),
        };
        let mut hits: Vec<Hit> = sessions
            .iter()
            .filter_map(|s| docs.get(s).map(|d| Self::query_session(d, s, query, limit)))
            .flatten()
            .collect();
        sort_hits(&mut hits);
        hits.truncate(limit);
        Ok(hits)
    }
}

impl TxIndexer for FakeSearch {
    fn prepare(&self, _conn: &Connection) -> Result<(), SearchError> {
        Ok(())
    }

    fn index_in(&self, _conn: &Connection, doc: &Doc) -> Result<(), SearchError> {
        self.index(doc)
    }

    fn remove_in(
        &self,
        _conn: &Connection,
        session: &SessionId,
        id: &DocId,
    ) -> Result<RemoveReport, SearchError> {
        self.remove(session, id)
    }
}
