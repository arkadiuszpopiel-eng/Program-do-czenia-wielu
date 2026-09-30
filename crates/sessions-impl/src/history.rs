//! Historia: drzewo tur append-only w bazie sesji.

use chrono::Utc;
use lib_sqlstore::rusqlite::{Connection, params};
use lib_sqlstore::unix_millis;
use search_contract::{Doc, DocId, DocKind};
use serde_json::json;
use sessions_contract::{
    BranchId, HeardPrefix, NewTurn, Role, SessionError, SessionHistory, SessionId, Siblings, Turn,
    TurnId, events, validate_heard_prefix, validate_new_turn,
};

use crate::SqliteSessions;
use crate::rows::{
    ACTIVE_LEAF, DRAFT, StoredTurn, TURN_COLUMNS, TURN_JOINS, children, db_err, load_turn,
    state_get, state_set, to_i64, to_u64, turn_from_row,
};
use sessions_contract::SessionDbProvider;

/// Gdzie dopisać turę.
#[derive(Clone, Copy)]
enum Target {
    /// Kontynuacja liścia (albo pierwsza tura, gdy `None`).
    Append(Option<TurnId>),
    /// Wariant tury (ten sam rodzic, nowa gałąź).
    Fork(TurnId),
}

fn role_str(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
        Role::Tool => "tool",
    }
}

fn new_branch(conn: &Connection, base: Option<TurnId>) -> Result<BranchId, SessionError> {
    conn.execute(
        "INSERT INTO branches(base_turn_id, created_at) VALUES (?1, ?2)",
        params![base.map(|b| to_i64(b.0)), unix_millis()],
    )
    .map_err(db_err)?;
    Ok(BranchId(to_u64(conn.last_insert_rowid())))
}

fn turn_count(conn: &Connection) -> Result<u64, SessionError> {
    conn.query_row("SELECT count(*) FROM turns", [], |r| r.get::<_, i64>(0))
        .map(to_u64)
        .map_err(db_err)
}

impl SqliteSessions {
    fn write_turn(
        &self,
        id: &SessionId,
        target: Target,
        new: NewTurn,
    ) -> Result<Turn, SessionError> {
        validate_new_turn(&new)?;
        let db = self.session_db(id)?;
        let (turn, branched) = db.with(|conn| {
            let tx = conn.transaction().map_err(db_err)?;
            let (parent, branch, branched) = match target {
                Target::Append(None) => {
                    if turn_count(&tx)? > 0 {
                        return Err(SessionError::RootExists);
                    }
                    (None, new_branch(&tx, None)?, true)
                }
                Target::Append(Some(p)) => {
                    let branch = load_turn(&tx, p)?.branch;
                    if !children(&tx, Some(p))?.is_empty() {
                        return Err(SessionError::NotALeaf { turn: p });
                    }
                    (Some(p), branch, false)
                }
                Target::Fork(sibling) => {
                    let parent = load_turn(&tx, sibling)?.parent;
                    (parent, new_branch(&tx, parent)?, true)
                }
            };
            let stored = StoredTurn {
                role: new.role,
                author: new.author,
                content: new.content,
                usage: new.usage,
                created_at: Utc::now(),
            };
            let body = serde_json::to_vec(&stored).map_err(SessionError::storage)?;
            tx.execute(
                "INSERT INTO turns(parent_id, branch_id, role, created_at, body)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    parent.map(|p| to_i64(p.0)),
                    to_i64(branch.0),
                    role_str(stored.role),
                    stored.created_at.timestamp_millis(),
                    body
                ],
            )
            .map_err(db_err)?;
            let turn_id = TurnId(to_u64(tx.last_insert_rowid()));
            if let Some(prefix) = new.heard_prefix {
                insert_heard(&tx, turn_id, prefix)?;
            }
            state_set(&tx, ACTIVE_LEAF, Some(&turn_id.0.to_string()))?;
            if let Some(indexer) = &self.indexer {
                let text = stored.content.searchable_text();
                if !text.is_empty() {
                    let doc = Doc {
                        id: DocId::new(DocKind::Turn, turn_id.0.to_string()),
                        session: id.clone(),
                        text,
                        ts: stored.created_at,
                    };
                    indexer.index_in(&tx, &doc).map_err(SessionError::storage)?;
                }
            }
            let turn = load_turn(&tx, turn_id)?;
            tx.commit().map_err(db_err)?;
            Ok((turn, branched))
        })?;
        self.bump_counters(id, turn.role != Role::User, turn.created_at)?;
        let payload = json!({
            "session": id, "turn": turn.id, "parent": turn.parent,
            "branch": turn.branch, "role": role_str(turn.role),
        });
        self.outbox.emit(events::TURN_APPENDED, id, payload);
        if branched && turn.branch.0 > 1 {
            let payload = json!({ "session": id, "branch": turn.branch, "turn": turn.id });
            self.outbox.emit(events::BRANCH_CREATED, id, payload);
        }
        Ok(turn)
    }

    fn read<R>(
        &self,
        id: &SessionId,
        f: impl FnOnce(&Connection) -> Result<R, SessionError>,
    ) -> Result<R, SessionError> {
        self.session_db(id)?.with(|conn| f(conn))
    }
}

fn insert_heard(conn: &Connection, turn: TurnId, prefix: HeardPrefix) -> Result<(), SessionError> {
    conn.execute(
        "INSERT INTO turn_heard(turn_id, chars, approximate, recorded_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            to_i64(turn.0),
            i64::try_from(prefix.chars).unwrap_or(i64::MAX),
            prefix.approximate,
            unix_millis()
        ],
    )
    .map(|_| ())
    .map_err(db_err)
}

impl SessionHistory for SqliteSessions {
    fn append_turn(
        &self,
        id: &SessionId,
        parent: Option<TurnId>,
        turn: NewTurn,
    ) -> Result<Turn, SessionError> {
        self.write_turn(id, Target::Append(parent), turn)
    }

    fn fork_from(
        &self,
        id: &SessionId,
        sibling_of: TurnId,
        turn: NewTurn,
    ) -> Result<Turn, SessionError> {
        self.write_turn(id, Target::Fork(sibling_of), turn)
    }

    fn turn(&self, id: &SessionId, turn: TurnId) -> Result<Turn, SessionError> {
        self.read(id, |c| load_turn(c, turn))
    }

    fn branch_projection(&self, id: &SessionId, leaf: TurnId) -> Result<Vec<Turn>, SessionError> {
        self.read(id, |c| {
            let sql = format!(
                "WITH RECURSIVE chain(id, depth) AS (
                     SELECT id, 0 FROM turns WHERE id = ?1
                     UNION ALL
                     SELECT t.parent_id, chain.depth + 1 FROM turns t JOIN chain ON t.id = chain.id
                     WHERE t.parent_id IS NOT NULL
                 )
                 SELECT {TURN_COLUMNS} FROM chain JOIN turns t ON t.id = chain.id {TURN_JOINS}
                 ORDER BY chain.depth DESC"
            );
            let mut stmt = c.prepare_cached(&sql).map_err(db_err)?;
            let turns = stmt
                .query_map(params![to_i64(leaf.0)], turn_from_row)
                .map_err(db_err)?
                .collect::<Result<Vec<Turn>, _>>()
                .map_err(db_err)?;
            if turns.is_empty() {
                return Err(SessionError::TurnNotFound { turn: leaf });
            }
            Ok(turns)
        })
    }

    fn siblings(&self, id: &SessionId, turn: TurnId) -> Result<Siblings, SessionError> {
        self.read(id, |c| {
            let parent = load_turn(c, turn)?.parent;
            let turns = children(c, parent)?;
            let index = turns.iter().position(|t| *t == turn).unwrap_or_default();
            Ok(Siblings { turns, index })
        })
    }

    fn latest_leaf(&self, id: &SessionId, from: TurnId) -> Result<TurnId, SessionError> {
        self.read(id, |c| {
            let mut current = load_turn(c, from)?.id;
            while let Some(child) = children(c, Some(current))?.last().copied() {
                current = child;
            }
            Ok(current)
        })
    }

    fn set_active_leaf(&self, id: &SessionId, leaf: TurnId) -> Result<(), SessionError> {
        self.read(id, |c| {
            load_turn(c, leaf)?;
            state_set(c, ACTIVE_LEAF, Some(&leaf.0.to_string()))
        })
    }

    fn active_leaf(&self, id: &SessionId) -> Result<Option<TurnId>, SessionError> {
        self.read(id, |c| {
            Ok(state_get(c, ACTIVE_LEAF)?
                .and_then(|v| v.parse::<u64>().ok())
                .map(TurnId))
        })
    }

    fn record_heard_prefix(
        &self,
        id: &SessionId,
        turn: TurnId,
        prefix: HeardPrefix,
    ) -> Result<Turn, SessionError> {
        self.read(id, |c| {
            let stored = load_turn(c, turn)?;
            if stored.heard_prefix.is_some() {
                return Err(SessionError::HeardPrefixAlreadyRecorded { turn });
            }
            validate_heard_prefix(stored.role, &stored.content, prefix)?;
            insert_heard(c, turn, prefix)?;
            load_turn(c, turn)
        })
    }

    fn set_hidden(&self, id: &SessionId, turn: TurnId, hidden: bool) -> Result<(), SessionError> {
        self.read(id, |c| {
            load_turn(c, turn)?;
            let sql = if hidden {
                "INSERT OR IGNORE INTO turn_hidden(turn_id, hidden_at) VALUES (?1, ?2)"
            } else {
                "DELETE FROM turn_hidden WHERE turn_id = ?1 AND ?2 IS NOT NULL"
            };
            c.execute(sql, params![to_i64(turn.0), unix_millis()])
                .map(|_| ())
                .map_err(db_err)
        })
    }

    fn save_draft(&self, id: &SessionId, text: &str) -> Result<(), SessionError> {
        self.read(id, |c| {
            state_set(c, DRAFT, (!text.is_empty()).then_some(text))
        })
    }

    fn draft(&self, id: &SessionId) -> Result<Option<String>, SessionError> {
        self.read(id, |c| state_get(c, DRAFT))
    }

    fn turn_count(&self, id: &SessionId) -> Result<u64, SessionError> {
        self.read(id, turn_count)
    }
}
