//! Odczyt/zapis wierszy: treść tury jako kanoniczny JSON (`body`), metadane sesji jako JSON.

use chrono::{DateTime, Utc};
use lib_sqlstore::rusqlite::{self, Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use sessions_contract::{
    Author, BranchId, HeardPrefix, ModelUsage, Role, SessionError, Turn, TurnContent, TurnId,
};

/// Niezmienna część tury zapisywana bajtowo w kolumnie `body` (nigdy aktualizowana).
#[derive(Debug, Serialize, Deserialize)]
pub struct StoredTurn {
    pub role: Role,
    pub author: Author,
    pub content: TurnContent,
    pub usage: Option<ModelUsage>,
    pub created_at: DateTime<Utc>,
}

/// Kolumny zapytania o turę (w tej kolejności czyta [`turn_from_row`]).
pub const TURN_COLUMNS: &str = "t.id, t.parent_id, t.branch_id, t.body, h.chars, h.approximate, \
     (SELECT 1 FROM turn_hidden x WHERE x.turn_id = t.id)";

/// Złączenia potrzebne dla [`TURN_COLUMNS`].
pub const TURN_JOINS: &str = "LEFT JOIN turn_heard h ON h.turn_id = t.id";

/// Błąd SQLite → błąd magazynu. Zapis do bazy tylko do odczytu (baza z nowszej wersji Alfy po
/// powrocie do starszej — `lib_sqlstore::migrate`, fala 5, ADR 0007) → czytelny komunikat.
pub fn db_err(e: rusqlite::Error) -> SessionError {
    if e.sqlite_error_code() == Some(rusqlite::ErrorCode::ReadOnly) {
        return SessionError::storage(
            "baza sesji pochodzi z nowszej wersji Alfy i jest tylko do odczytu — zaktualizuj Alfę, \
             żeby pisać dalej",
        );
    }
    SessionError::storage(e)
}

pub fn to_u64(v: i64) -> u64 {
    u64::try_from(v).unwrap_or_default()
}

pub fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// Mapuje wiersz na [`Turn`] (błąd dekodowania JSON → błąd konwersji rusqlite).
pub fn turn_from_row(row: &Row<'_>) -> rusqlite::Result<Turn> {
    let body: Vec<u8> = row.get(3)?;
    let stored: StoredTurn = serde_json::from_slice(&body).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Blob, Box::new(e))
    })?;
    let heard = match (
        row.get::<_, Option<i64>>(4)?,
        row.get::<_, Option<bool>>(5)?,
    ) {
        (Some(chars), Some(approximate)) => Some(HeardPrefix {
            chars: usize::try_from(chars).unwrap_or_default(),
            approximate,
        }),
        _ => None,
    };
    Ok(Turn {
        id: TurnId(to_u64(row.get(0)?)),
        parent: row.get::<_, Option<i64>>(1)?.map(|p| TurnId(to_u64(p))),
        branch: BranchId(to_u64(row.get(2)?)),
        role: stored.role,
        author: stored.author,
        content: stored.content,
        usage: stored.usage,
        created_at: stored.created_at,
        heard_prefix: heard,
        hidden: row.get::<_, Option<i64>>(6)?.is_some(),
    })
}

/// Tura albo [`SessionError::TurnNotFound`].
pub fn load_turn(conn: &Connection, turn: TurnId) -> Result<Turn, SessionError> {
    conn.query_row(
        &format!("SELECT {TURN_COLUMNS} FROM turns t {TURN_JOINS} WHERE t.id = ?1"),
        params![to_i64(turn.0)],
        turn_from_row,
    )
    .optional()
    .map_err(db_err)?
    .ok_or(SessionError::TurnNotFound { turn })
}

/// Dzieci tury (`None` = korzenie), rosnąco po `id`.
pub fn children(conn: &Connection, parent: Option<TurnId>) -> Result<Vec<TurnId>, SessionError> {
    let mut stmt = conn
        .prepare_cached("SELECT id FROM turns WHERE parent_id IS ?1 ORDER BY id")
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![parent.map(|p| to_i64(p.0))], |r| r.get::<_, i64>(0))
        .map_err(db_err)?;
    rows.map(|r| r.map(|id| TurnId(to_u64(id))).map_err(db_err))
        .collect()
}

/// Wartość z `session_state`.
pub fn state_get(conn: &Connection, key: &str) -> Result<Option<String>, SessionError> {
    conn.query_row(
        "SELECT value FROM session_state WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .optional()
    .map_err(db_err)
}

/// Zapis/usunięcie wartości w `session_state` (`None` usuwa).
pub fn state_set(conn: &Connection, key: &str, value: Option<&str>) -> Result<(), SessionError> {
    match value {
        Some(v) => conn.execute(
            "INSERT INTO session_state(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, v],
        ),
        None => conn.execute("DELETE FROM session_state WHERE key = ?1", params![key]),
    }
    .map(|_| ())
    .map_err(db_err)
}

/// Klucz stanu: aktywny liść.
pub const ACTIVE_LEAF: &str = "active_leaf";
/// Klucz stanu: szkic composera.
pub const DRAFT: &str = "draft";
