//! Zapytanie w jednej bazie sesji: FTS5 (bm25), kNN `vec0` (kosinus), hybryda RRF.

use lib_sqlstore::rusqlite::{Connection, params};
use lib_sqlstore::{fts5_match, search_tokens, vector_to_blob};
use search_contract::{
    ConnQuery, DocId, DocKind, Embedder, Hit, Mode, Query, SearchError, SessionId, fuse_rrf,
    make_snippet,
};

use crate::index::{embed_one, storage, vec_table};

/// Lista rankingowa: `(rowid, wynik)` malejąco po wyniku.
type Ranked = Vec<(i64, f32)>;

/// Zapytanie rozłożone na części (wspólne dla [`Query`] i [`ConnQuery`]).
pub struct Parts<'a> {
    /// Tekst FTS.
    pub fts_text: &'a str,
    /// Tekst embeddingu.
    pub vector_text: &'a str,
    /// Tryb.
    pub mode: Mode,
    /// Rodzaje (już przefiltrowane).
    pub kinds: Vec<DocKind>,
    /// FTS: dowolne słowo (OR) zamiast wszystkich (AND).
    pub match_any: bool,
}

impl<'a> Parts<'a> {
    /// Części zapytania użytkownika (`Query`).
    pub fn of_query(q: &'a Query) -> Self {
        Self {
            fts_text: &q.text,
            vector_text: &q.text,
            mode: q.mode,
            kinds: DocKind::ALL.into_iter().filter(|k| q.accepts(*k)).collect(),
            match_any: false,
        }
    }

    /// Części zapytania w połączeniu wywołującego (`ConnQuery`).
    pub fn of_conn(q: &'a ConnQuery) -> Self {
        Self {
            fts_text: &q.text,
            vector_text: q.vector_text(),
            mode: q.mode,
            kinds: DocKind::ALL.into_iter().filter(|k| q.accepts(*k)).collect(),
            match_any: q.match_any,
        }
    }
}

/// Wyrażenie FTS5 „dowolne słowo”: `"a"* OR "b"*` (słowa cytowane — składnia użytkownika nie
/// przechodzi).
fn fts5_match_any(text: &str) -> Option<String> {
    let terms = search_tokens(text);
    if terms.is_empty() {
        return None;
    }
    let parts: Vec<String> = terms
        .iter()
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    Some(parts.join(" OR "))
}

fn fts_list(conn: &Connection, q: &Parts<'_>, n: usize) -> Result<Ranked, SearchError> {
    let expr = if q.match_any {
        fts5_match_any(q.fts_text)
    } else {
        fts5_match(q.fts_text)
    };
    let Some(expr) = expr else {
        return Ok(Vec::new());
    };
    let kinds: String = q
        .kinds
        .iter()
        .map(|k| format!(",{},", k.as_str()))
        .collect();
    let mut stmt = conn
        .prepare_cached(
            "SELECT d.id, bm25(search_fts) FROM search_fts JOIN search_docs d ON d.id = search_fts.rowid
             WHERE search_fts MATCH ?1 AND instr(?2, ',' || d.kind || ',') > 0
             ORDER BY bm25(search_fts), d.id LIMIT ?3",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(
            params![expr, kinds, i64::try_from(n).unwrap_or(i64::MAX)],
            |r| Ok((r.get::<_, i64>(0)?, -(r.get::<_, f64>(1)? as f32))),
        )
        .map_err(storage)?;
    rows.collect::<Result<_, _>>().map_err(storage)
}

fn vector_list(
    conn: &Connection,
    embedder: &dyn Embedder,
    q: &Parts<'_>,
    n: usize,
) -> Result<Ranked, SearchError> {
    let blob = vector_to_blob(&embed_one(embedder, q.vector_text)?);
    let mut all: Vec<(i64, f64)> = Vec::new();
    for kind in q.kinds.iter().copied() {
        let sql = format!(
            "SELECT rowid, distance FROM {} WHERE embedding MATCH ?1 AND k = ?2",
            vec_table(kind)
        );
        let mut stmt = conn.prepare_cached(&sql).map_err(storage)?;
        let rows = stmt
            .query_map(params![blob, i64::try_from(n).unwrap_or(i64::MAX)], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
            })
            .map_err(storage)?;
        for row in rows {
            all.push(row.map_err(storage)?);
        }
    }
    all.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    all.truncate(n);
    Ok(all
        .into_iter()
        .map(|(id, d)| (id, 1.0 - d as f32))
        .collect())
}

fn load_doc(conn: &Connection, rowid: i64) -> Result<Option<(DocId, String)>, SearchError> {
    let mut stmt = conn
        .prepare_cached("SELECT kind, key, text FROM search_docs WHERE id = ?1")
        .map_err(storage)?;
    let mut rows = stmt.query(params![rowid]).map_err(storage)?;
    let Some(row) = rows.next().map_err(storage)? else {
        return Ok(None);
    };
    let kind: String = row.get(0).map_err(storage)?;
    let key: String = row.get(1).map_err(storage)?;
    let text: String = row.get(2).map_err(storage)?;
    Ok(DocKind::parse(&kind).map(|k| (DocId::new(k, key), text)))
}

/// Trafienia w jednej bazie (co najwyżej `limit`), deterministycznie posortowane.
pub fn query_conn(
    conn: &Connection,
    embedder: &dyn Embedder,
    session: &SessionId,
    q: &Parts<'_>,
    limit: usize,
    snippet_chars: usize,
) -> Result<Vec<Hit>, SearchError> {
    let candidates = (limit * 4).max(20);
    let ranked: Ranked = match q.mode {
        Mode::Fts => fts_list(conn, q, candidates)?,
        Mode::Vector => vector_list(conn, embedder, q, candidates)?,
        Mode::Hybrid => {
            let fts = fts_list(conn, q, candidates)?;
            let vec = vector_list(conn, embedder, q, candidates)?;
            fuse_rrf(&[
                fts.into_iter().map(|(id, _)| id).collect::<Vec<_>>(),
                vec.into_iter().map(|(id, _)| id).collect::<Vec<_>>(),
            ])
        }
    };
    let mut hits = Vec::with_capacity(limit.min(ranked.len()));
    for (rowid, score) in ranked.into_iter().take(limit) {
        if let Some((doc, text)) = load_doc(conn, rowid)? {
            hits.push(Hit {
                snippet: make_snippet(&text, q.vector_text, snippet_chars),
                doc,
                session: session.clone(),
                score,
            });
        }
    }
    Ok(hits)
}
