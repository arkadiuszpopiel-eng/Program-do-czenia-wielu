//! Krok przebudowy wektorów jednej bazy w trzech fazach: (1) w blokadzie — stan i partia dokumentów
//! (powyżej kursora przebudowy albo z `search_vec_missing`); (2) **bez blokady** — embedding partii;
//! (3) w blokadzie, w transakcji — zapis, jeśli stan się nie zmienił, z pominięciem dokumentów
//! zmienionych/usuniętych w międzyczasie, przesunięcie kursora, koniec przebudowy.

use lib_sqlstore::Db;
use lib_sqlstore::rusqlite::{Connection, OptionalExtension, params};
use search_contract::{DocKind, Embedder, ReindexProgress, SearchError, VectorStatus};

use crate::index::{check_vectors, mark_missing, put_vector};
use crate::state::{VecState, finish_rebuild, prepare, set_meta, storage};

/// Dokument do przeliczenia: `(id, rodzaj, tekst)`.
type Work = Vec<(i64, DocKind, String)>;

fn count(
    conn: &Connection,
    sql: &str,
    p: &[&dyn lib_sqlstore::rusqlite::ToSql],
) -> Result<u64, SearchError> {
    conn.query_row(sql, p, |r| r.get::<_, i64>(0))
        .map(|n| u64::try_from(n).unwrap_or(0))
        .map_err(storage)
}

/// Stan wektorów (dla UI i decyzji o przebudowie).
pub fn status(conn: &Connection, state: &VecState) -> Result<VectorStatus, SearchError> {
    let total = count(conn, "SELECT count(*) FROM search_docs", &[])?;
    Ok(match &state.target {
        None => VectorStatus::Ready {
            embedder: state.active.clone(),
            missing: count(
                conn,
                "SELECT count(*) FROM search_vec_missing WHERE gen = ?1",
                &[&state.active_gen],
            )?,
        },
        Some((_, to, cursor)) => VectorStatus::Rebuilding {
            from: state.active.clone(),
            to: to.clone(),
            done: count(
                conn,
                "SELECT count(*) FROM search_docs WHERE id <= ?1",
                &[cursor],
            )?,
            total,
        },
    })
}

fn progress(
    conn: &Connection,
    state: &VecState,
    embedded: u64,
) -> Result<ReindexProgress, SearchError> {
    Ok(match status(conn, state)? {
        VectorStatus::Ready { missing, .. } => {
            let total = count(conn, "SELECT count(*) FROM search_docs", &[])?;
            ReindexProgress {
                embedded,
                done: total.saturating_sub(missing),
                total,
                finished: missing == 0,
            }
        }
        VectorStatus::Rebuilding { done, total, .. } => ReindexProgress {
            embedded,
            done,
            total,
            finished: false,
        },
    })
}

fn select_work(conn: &Connection, state: &VecState, batch: usize) -> Result<Work, SearchError> {
    let limit = i64::try_from(batch.max(1)).unwrap_or(i64::MAX);
    let (sql, key) = match &state.target {
        Some((_, _, cursor)) => (
            "SELECT id, kind, text FROM search_docs WHERE id > ?1 ORDER BY id LIMIT ?2",
            *cursor,
        ),
        None => (
            "SELECT d.id, d.kind, d.text FROM search_vec_missing m JOIN search_docs d ON d.id = m.id
             WHERE m.gen = ?1 ORDER BY d.id LIMIT ?2",
            state.active_gen,
        ),
    };
    let mut stmt = conn.prepare_cached(sql).map_err(storage)?;
    let rows = stmt
        .query_map(params![key, limit], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(storage)?;
    let mut work = Vec::new();
    for row in rows {
        let (id, kind, text) = row.map_err(storage)?;
        if let Some(kind) = DocKind::parse(&kind) {
            work.push((id, kind, text));
        }
    }
    Ok(work)
}

/// Faza 1: stan i partia; pusta partia przy przebudowie → zakończenie przebudowy (w transakcji).
fn plan(db: &Db, embedder: &dyn Embedder, batch: usize) -> Result<(VecState, Work), SearchError> {
    db.with(|conn| {
        let tx = conn.transaction().map_err(storage)?;
        let mut state = prepare(&tx, embedder)?;
        let mut work = select_work(&tx, &state, batch)?;
        if work.is_empty() && state.target.is_some() {
            finish_rebuild(&tx, &state)?;
            state = prepare(&tx, embedder)?;
            work = select_work(&tx, &state, batch)?;
        }
        tx.commit().map_err(storage)?;
        Ok((state, work))
    })
}

/// Faza 2 (bez blokady): wektory partii. Błąd całej partii → próba dokument po dokumencie;
/// dokument, który wciąż zawodzi, zostaje bez wektora (`None` → `search_vec_missing`), żeby jeden
/// „trujący” tekst nie zatrzymał przebudowy. Gdy zawodzą wszystkie — embedder niedostępny (błąd,
/// kursor stoi).
fn embed_work(embedder: &dyn Embedder, work: &Work) -> Result<Vec<Option<Vec<f32>>>, SearchError> {
    let texts: Vec<&str> = work.iter().map(|(_, _, t)| t.as_str()).collect();
    let first = embedder
        .embed(&texts)
        .and_then(|v| check_vectors(embedder, v, texts.len()));
    let err = match first {
        Ok(vectors) => return Ok(vectors.into_iter().map(Some).collect()),
        Err(e) if texts.len() == 1 => return Err(e),
        Err(e) => e,
    };
    let single: Vec<Option<Vec<f32>>> = texts
        .iter()
        .map(|t| {
            embedder
                .embed(&[t])
                .and_then(|v| check_vectors(embedder, v, 1))
                .ok()
                .and_then(|mut v| v.pop())
        })
        .collect();
    if single.iter().all(Option::is_none) {
        Err(err)
    } else {
        Ok(single)
    }
}

/// Jeden krok przebudowy (≤ `batch` dokumentów).
pub fn step(
    db: &Db,
    embedder: &dyn Embedder,
    batch: usize,
) -> Result<ReindexProgress, SearchError> {
    let (state, work) = plan(db, embedder, batch)?;
    if work.is_empty() {
        return db.with(|conn| progress(conn, &prepare(conn, embedder)?, 0));
    }
    let vectors = embed_work(embedder, &work)?;
    db.with(|conn| {
        let tx = conn.transaction().map_err(storage)?;
        let now = prepare(&tx, embedder)?;
        if !now.same(&state) {
            // Stan zmienił się w trakcie embeddingu (inny krok, zmiana embeddera) — bez zapisu.
            let p = progress(&tx, &now, 0)?;
            tx.commit().map_err(storage)?;
            return Ok(p);
        }
        let generation = state.write_gen();
        let mut embedded = 0_u64;
        for ((id, kind, text), vector) in work.iter().zip(&vectors) {
            let current: Option<String> = tx
                .query_row(
                    "SELECT text FROM search_docs WHERE id = ?1",
                    params![id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            if current.as_deref() != Some(text.as_str()) {
                continue;
            }
            match vector {
                Some(v) => {
                    put_vector(&tx, *kind, generation, *id, v)?;
                    embedded += 1;
                }
                None => mark_missing(&tx, *id, generation)?,
            }
        }
        if let (Some(_), Some((last, _, _))) = (&state.target, work.last()) {
            set_meta(&tx, "cursor", &last.to_string())?;
        }
        let after = prepare(&tx, embedder)?;
        let p = progress(&tx, &after, embedded)?;
        tx.commit().map_err(storage)?;
        Ok(p)
    })
}
