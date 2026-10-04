//! Atrapa modułu `search` (docs/modules/search/SPEC.md, „Fake”).
//!
//! - [`FakeSearch`] — indeks w pamięci: dopasowanie słów (bez diakrytyków, prefiksy, AND),
//!   kosinus na wektorach [`HashEmbedder`], hybryda RRF; te same reguły autoryzacji i fragmentów
//!   co `search-impl` (wspólne funkcje z `search-contract`).
//! - [`HashEmbedder`] — deterministyczny embedder (hash n-gramów → 64 wym.); inny embedder przez
//!   [`FakeSearch::with_embedder`] (np. eval F7-02 z prawdziwym modelem ONNX z `lib-embed`):
//!   dokumenty przez `embed`, zapytania przez `embed_query`; błąd embeddera → dokument bez wektora,
//!   zapytanie bez wektora spada do FTS (jak `search-impl`).
//! - [`RecordingIndexer`] — `TxIndexer`, który tylko zapisuje wywołania (testy `sessions`/`memory`).
//!
//! `FakeSearch` implementuje też `TxIndexer` i `TxSearcher` (ignoruje połączenie, indeksuje i pyta
//! w pamięci po etykiecie bazy = `Doc::session`) — dzięki temu
//! moduł zapisujący dane (np. `memory-impl`) można testować z jedną spójną atrapą indeksu.
//! Uwaga: indeks atrapy nie jest wycofywany razem z transakcją wywołującego.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod embedder;
mod indexer;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lib_sqlstore::rusqlite::Connection;
use lib_sqlstore::{search_tokens, tokenize};
use search_contract::{
    Caller, ConnQuery, DEFAULT_SNIPPET_CHARS, Doc, DocId, DocKind, Embedder, Hit, MAX_LIMIT, Mode,
    Query, RemoveReport, Search, SearchError, SessionId, SessionSet, TxIndexer, TxSearcher,
    authorize, fuse_rrf, make_snippet, sort_hits,
};

pub use embedder::{HASH_DIMS, HashEmbedder, cosine};
pub use indexer::RecordingIndexer;

#[derive(Debug, Clone)]
struct Entry {
    text: String,
    terms: Vec<String>,
    /// `None` = embedder niedostępny przy zapisie (dokument tylko w FTS).
    vector: Option<Vec<f32>>,
}

/// Indeks w pamięci: `(sesja, dokument) → tekst + wektor`.
#[derive(Default)]
pub struct FakeSearch {
    docs: Mutex<BTreeMap<SessionId, BTreeMap<DocId, Entry>>>,
    embedder: Option<Arc<dyn Embedder>>,
}

impl std::fmt::Debug for FakeSearch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeSearch")
            .field(
                "embedder",
                &self.embedder.as_ref().map(|e| e.model_id().to_owned()),
            )
            .finish_non_exhaustive()
    }
}

fn first_vector(out: Result<Vec<Vec<f32>>, SearchError>, dims: usize) -> Option<Vec<f32>> {
    out.ok()
        .and_then(|mut v| v.pop())
        .filter(|v| v.len() == dims)
}

impl FakeSearch {
    /// Pusty indeks (wektory `HashEmbedder`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Pusty indeks z podanym embedderem.
    pub fn with_embedder(embedder: Arc<dyn Embedder>) -> Self {
        Self {
            docs: Mutex::default(),
            embedder: Some(embedder),
        }
    }

    fn passage_vector(&self, text: &str) -> Option<Vec<f32>> {
        match &self.embedder {
            Some(e) => first_vector(e.embed(&[text]), e.dims()),
            None => Some(HashEmbedder::vector(text)),
        }
    }

    fn query_vector(&self, text: &str) -> Option<Vec<f32>> {
        match &self.embedder {
            Some(e) => first_vector(e.embed_query(&[text]), e.dims()),
            None => Some(HashEmbedder::vector(text)),
        }
    }

    /// Liczba dokumentów w sesji.
    pub fn doc_count(&self, session: &SessionId) -> usize {
        self.lock().get(session).map_or(0, BTreeMap::len)
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<SessionId, BTreeMap<DocId, Entry>>> {
        self.docs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn query_session(
        &self,
        docs: &BTreeMap<DocId, Entry>,
        session: &SessionId,
        q: &Parts<'_>,
        limit: usize,
    ) -> Vec<Hit> {
        let terms = search_tokens(q.fts_text);
        let candidates = (limit * 4).max(20);
        let accepted = || docs.iter().filter(|(id, _)| q.kinds.contains(&id.kind));
        let mut fts: Vec<(DocId, f32)> = if terms.is_empty() {
            Vec::new()
        } else {
            accepted()
                .filter_map(|(id, e)| {
                    fts_score(&terms, &e.terms, q.match_any).map(|s| (id.clone(), s))
                })
                .collect()
        };
        fts.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        fts.truncate(candidates);
        let query_vec = match q.mode {
            Mode::Fts => None,
            Mode::Vector | Mode::Hybrid => self.query_vector(q.vector_text),
        };
        let ranked: Vec<(DocId, f32)> = match (q.mode, query_vec) {
            (Mode::Fts, _) | (_, None) => fts,
            (mode, Some(query_vec)) => {
                let mut vec: Vec<(DocId, f32)> = accepted()
                    .filter_map(|(id, e)| {
                        e.vector
                            .as_ref()
                            .map(|v| (id.clone(), cosine(&query_vec, v)))
                    })
                    .collect();
                vec.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                vec.truncate(candidates);
                if mode == Mode::Vector {
                    vec
                } else {
                    fuse_rrf(&[
                        fts.into_iter().map(|(k, _)| k).collect(),
                        vec.into_iter().map(|(k, _)| k).collect(),
                    ])
                }
            }
        };
        ranked
            .into_iter()
            .take(limit)
            .filter_map(|(id, score)| {
                docs.get(&id).map(|e| Hit {
                    snippet: make_snippet(&e.text, q.vector_text, DEFAULT_SNIPPET_CHARS),
                    doc: id,
                    session: session.clone(),
                    score,
                })
            })
            .collect()
    }
}

/// Zapytanie rozłożone na części (wspólne dla `Query` i `ConnQuery`).
struct Parts<'a> {
    fts_text: &'a str,
    vector_text: &'a str,
    mode: Mode,
    kinds: Vec<DocKind>,
    match_any: bool,
}

impl<'a> Parts<'a> {
    fn of_query(q: &'a Query) -> Self {
        Self {
            fts_text: &q.text,
            vector_text: &q.text,
            mode: q.mode,
            kinds: DocKind::ALL.into_iter().filter(|k| q.accepts(*k)).collect(),
            match_any: false,
        }
    }

    fn of_conn(q: &'a ConnQuery) -> Self {
        Self {
            fts_text: &q.text,
            vector_text: q.vector_text(),
            mode: q.mode,
            kinds: DocKind::ALL.into_iter().filter(|k| q.accepts(*k)).collect(),
            match_any: q.match_any,
        }
    }
}

/// Wynik FTS atrapy: słowo zapytania pasuje, gdy jest prefiksem słowa dokumentu; `any = false` —
/// wszystkie słowa muszą pasować (AND), `true` — dowolne (OR); wynik = liczba pasujących wystąpień.
fn fts_score(query: &[String], doc: &[String], any: bool) -> Option<f32> {
    let mut total = 0_u16;
    for q in query {
        let n = doc.iter().filter(|t| t.starts_with(q.as_str())).count();
        if n == 0 && !any {
            return None;
        }
        total = total.saturating_add(u16::try_from(n).unwrap_or(u16::MAX));
    }
    (total > 0).then_some(f32::from(total))
}

impl Search for FakeSearch {
    fn index(&self, doc: &Doc) -> Result<(), SearchError> {
        let entry = Entry {
            terms: tokenize(&doc.text).into_iter().map(|t| t.term).collect(),
            vector: self.passage_vector(&doc.text),
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
            .filter_map(|s| {
                docs.get(s)
                    .map(|d| self.query_session(d, s, &Parts::of_query(query), limit))
            })
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

impl TxSearcher for FakeSearch {
    fn query_in(
        &self,
        _conn: &Connection,
        label: &SessionId,
        query: &ConnQuery,
    ) -> Result<Vec<Hit>, SearchError> {
        let limit = query.limit.min(MAX_LIMIT);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let docs = self.lock();
        let mut hits = docs
            .get(label)
            .map(|d| self.query_session(d, label, &Parts::of_conn(query), limit))
            .unwrap_or_default();
        sort_hits(&mut hits);
        Ok(hits)
    }
}
