//! Import historii z zachowanymi identyfikatorami (`transfer`, paczki `.alfa`): partia tur do
//! istniejącej sesji (`import_turns`) i cała sesja atomowo (`adopt_session`).

use std::sync::Arc;

use lib_sqlstore::rusqlite::{Connection, params};
use lib_sqlstore::unix_millis;
use sessions_contract::{
    BranchId, PortableSession, SessionDbProvider, SessionError, SessionId, TreeCursor, Turn,
    TurnId, load_or_create_key, session_key_name,
};

use crate::history::{insert_heard, role_str};
use crate::rows::{ACTIVE_LEAF, DRAFT, StoredTurn, db_err, state_set, to_i64, to_u64};
use crate::{SqliteSessions, lock};

impl SqliteSessions {
    /// Zapisuje tury (reguły [`TreeCursor`]) w bieżącej transakcji `conn`.
    fn write_turns(
        &self,
        conn: &Connection,
        id: &SessionId,
        cursor: &mut TreeCursor,
        turns: &[Turn],
    ) -> Result<(), SessionError> {
        for turn in turns {
            if cursor.accept(turn)? {
                conn.execute(
                    "INSERT INTO branches(id, base_turn_id, created_at) VALUES (?1, ?2, ?3)",
                    params![
                        to_i64(turn.branch.0),
                        turn.parent.map(|p| to_i64(p.0)),
                        turn.created_at.timestamp_millis()
                    ],
                )
                .map_err(db_err)?;
            }
            let stored = StoredTurn {
                role: turn.role,
                author: turn.author.clone(),
                content: turn.content.clone(),
                usage: turn.usage.clone(),
                created_at: turn.created_at,
            };
            let body = serde_json::to_vec(&stored).map_err(SessionError::storage)?;
            conn.execute(
                "INSERT INTO turns(id, parent_id, branch_id, role, created_at, body)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    to_i64(turn.id.0),
                    turn.parent.map(|p| to_i64(p.0)),
                    to_i64(turn.branch.0),
                    role_str(turn.role),
                    turn.created_at.timestamp_millis(),
                    body
                ],
            )
            .map_err(db_err)?;
            if let Some(prefix) = turn.heard_prefix {
                insert_heard(conn, turn.id, prefix)?;
            }
            if turn.hidden {
                conn.execute(
                    "INSERT INTO turn_hidden(turn_id, hidden_at) VALUES (?1, ?2)",
                    params![to_i64(turn.id.0), unix_millis()],
                )
                .map_err(db_err)?;
            }
            self.index_turn(conn, id, turn.id, &stored)?;
        }
        Ok(())
    }

    /// Import partii pełnych tur do istniejącej sesji (jedna transakcja).
    pub(crate) fn import_batch(&self, id: &SessionId, turns: &[Turn]) -> Result<u64, SessionError> {
        let db = self.session_db(id)?;
        db.with(|conn| {
            let tx = conn.transaction().map_err(db_err)?;
            let mut cursor = load_cursor(&tx)?;
            self.write_turns(&tx, id, &mut cursor, turns)?;
            tx.commit().map_err(db_err)?;
            Ok(turns.len() as u64)
        })
    }

    /// Zakłada bazę i klucz sesji, zapisuje historię i stan w jednej transakcji, a wpis katalogu
    /// dodaje **na końcu** — sesja istnieje w całości albo wcale (po awarii plik bez wpisu
    /// sprząta `sweep_orphans`). Przy błędzie usuwa klucz i pliki.
    pub(crate) fn insert_portable(&self, session: &PortableSession) -> Result<(), SessionError> {
        let id = &session.meta.id;
        let key = load_or_create_key(self.vault.as_ref(), &session_key_name(id))?;
        let mut open = lock(&self.open);
        let db = self.open_db(id, &key)?;
        let written = db
            .with(|conn| {
                let tx = conn.transaction().map_err(db_err)?;
                self.write_turns(&tx, id, &mut TreeCursor::empty(), &session.turns)?;
                if let Some(leaf) = session.active_leaf {
                    state_set(&tx, ACTIVE_LEAF, Some(&leaf.0.to_string()))?;
                }
                if let Some(draft) = session.draft.as_deref().filter(|d| !d.is_empty()) {
                    state_set(&tx, DRAFT, Some(draft))?;
                }
                tx.commit().map_err(db_err)
            })
            .and_then(|()| self.insert_catalog_row(session));
        if let Err(e) = written {
            drop(db);
            let _ = self.vault.delete(&session_key_name(id));
            let _ = lib_sqlstore::remove_database(&self.session_path(id));
            return Err(e);
        }
        open.insert(id.clone(), Arc::clone(&db));
        Ok(())
    }

    fn insert_catalog_row(&self, session: &PortableSession) -> Result<(), SessionError> {
        let meta = serde_json::to_string(&session.meta).map_err(SessionError::storage)?;
        let last = match session.last_turn_at() {
            Some(t) => Some(serde_json::to_string(&t).map_err(SessionError::storage)?),
            None => None,
        };
        self.index
            .with(|c| {
                c.execute(
                    "INSERT INTO sessions(id, meta, turns, unread, last_turn_at)
                     VALUES (?1, ?2, ?3, 0, ?4)",
                    params![
                        session.meta.id.as_str(),
                        meta,
                        to_i64(session.turns.len() as u64),
                        last
                    ],
                )
            })
            .map(|_| ())
            .map_err(db_err)
    }
}

/// Kursor drzewa z istniejących tur (tylko kolumny strukturalne).
fn load_cursor(conn: &Connection) -> Result<TreeCursor, SessionError> {
    let mut stmt = conn
        .prepare_cached("SELECT id, parent_id, branch_id FROM turns")
        .map_err(db_err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                TurnId(to_u64(r.get(0)?)),
                r.get::<_, Option<i64>>(1)?.map(|p| TurnId(to_u64(p))),
                BranchId(to_u64(r.get(2)?)),
            ))
        })
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;
    Ok(TreeCursor::from_entries(rows))
}
