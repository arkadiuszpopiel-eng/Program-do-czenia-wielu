//! Katalog sesji (`index.db`).

use std::sync::Arc;

use chrono::{DateTime, Utc};
use lib_sqlstore::rusqlite::{OptionalExtension, params};
use serde_json::json;
use sessions_contract::{
    DEFAULT_TITLE, DeleteReport, NewSession, PortableSession, SessionCatalog, SessionError,
    SessionId, SessionMeta, SessionPatch, SessionQuery, SessionSummary, apply_query, events,
    normalize_tags, session_key_name,
};

use crate::rows::{db_err, to_u64};
use crate::{SqliteSessions, lock};

fn meta_json(meta: &SessionMeta) -> Result<String, SessionError> {
    serde_json::to_string(meta).map_err(SessionError::storage)
}

fn parse_meta(text: &str) -> Result<SessionMeta, SessionError> {
    serde_json::from_str(text).map_err(SessionError::storage)
}

impl SqliteSessions {
    fn load_meta(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        let text: Option<String> = self
            .index
            .with(|c| {
                c.query_row(
                    "SELECT meta FROM sessions WHERE id = ?1",
                    [id.as_str()],
                    |r| r.get(0),
                )
                .optional()
            })
            .map_err(db_err)?;
        parse_meta(&text.ok_or_else(|| SessionError::NotFound { id: id.clone() })?)
    }

    fn save_meta(&self, meta: &SessionMeta) -> Result<(), SessionError> {
        let json = meta_json(meta)?;
        let changed = self
            .index
            .with(|c| {
                c.execute(
                    "UPDATE sessions SET meta = ?2 WHERE id = ?1",
                    params![meta.id.as_str(), json],
                )
            })
            .map_err(db_err)?;
        if changed == 0 {
            return Err(SessionError::NotFound {
                id: meta.id.clone(),
            });
        }
        Ok(())
    }

    fn modify(
        &self,
        id: &SessionId,
        f: impl FnOnce(&mut SessionMeta),
    ) -> Result<(SessionMeta, SessionMeta), SessionError> {
        let before = self.load_meta(id)?;
        let mut after = before.clone();
        f(&mut after);
        if after != before {
            self.save_meta(&after)?;
        }
        Ok((before, after))
    }

    /// Wszystkie sesje katalogu (bez filtrów).
    fn all_summaries(&self) -> Result<Vec<SessionSummary>, SessionError> {
        type Row = (String, i64, i64, Option<String>);
        let rows: Vec<Row> = self
            .index
            .with(|c| {
                let mut stmt = c.prepare_cached(
                    "SELECT meta, turns, unread, last_turn_at FROM sessions ORDER BY id",
                )?;
                let rows =
                    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
                rows.collect()
            })
            .map_err(db_err)?;
        let active = lock(&self.active).clone();
        let mut all = Vec::with_capacity(rows.len());
        for (meta, turns, unread, last) in rows {
            let meta = parse_meta(&meta)?;
            let last_turn_at = match last {
                Some(t) => Some(serde_json::from_str(&t).map_err(SessionError::storage)?),
                None => None,
            };
            all.push(SessionSummary {
                active: active.contains(&meta.id),
                meta,
                turns: to_u64(turns),
                unread: to_u64(unread),
                last_turn_at,
            });
        }
        Ok(all)
    }

    /// Po zapisie tury: liczniki listy w katalogu (osobna baza — poza transakcją tury).
    pub(crate) fn bump_counters(
        &self,
        id: &SessionId,
        unread: bool,
        at: DateTime<Utc>,
    ) -> Result<(), SessionError> {
        let at = serde_json::to_string(&at).map_err(SessionError::storage)?;
        self.index
            .with(|c| {
                c.execute(
                    "UPDATE sessions SET turns = turns + 1, unread = unread + ?2, last_turn_at = ?3
                     WHERE id = ?1",
                    params![id.as_str(), i64::from(unread), at],
                )
            })
            .map(|_| ())
            .map_err(db_err)
    }
}

impl SqliteSessions {
    /// Po imporcie tur: liczniki listy (`turns += count`, `last_turn_at` = późniejszy z dwóch).
    pub(crate) fn add_imported(
        &self,
        id: &SessionId,
        count: u64,
        latest: Option<DateTime<Utc>>,
    ) -> Result<(), SessionError> {
        let current: Option<String> = self
            .index
            .with(|c| {
                c.query_row(
                    "SELECT last_turn_at FROM sessions WHERE id = ?1",
                    [id.as_str()],
                    |r| r.get(0),
                )
            })
            .map_err(db_err)?;
        let current: Option<DateTime<Utc>> = match current {
            Some(t) => Some(serde_json::from_str(&t).map_err(SessionError::storage)?),
            None => None,
        };
        let last = current.max(latest);
        let last = match last {
            Some(t) => Some(serde_json::to_string(&t).map_err(SessionError::storage)?),
            None => None,
        };
        self.index
            .with(|c| {
                c.execute(
                    "UPDATE sessions SET turns = turns + ?2, last_turn_at = ?3 WHERE id = ?1",
                    params![id.as_str(), crate::rows::to_i64(count), last],
                )
            })
            .map(|_| ())
            .map_err(db_err)
    }
}

impl SessionCatalog for SqliteSessions {
    fn create_session(&self, new: NewSession) -> Result<SessionMeta, SessionError> {
        let id = SessionId::new(uuid::Uuid::now_v7().to_string());
        let taken: Vec<String> = self
            .all_summaries()?
            .iter()
            .map(|s| s.meta.workdir_name())
            .collect();
        let meta = SessionMeta::from_new(
            new,
            id.clone(),
            Utc::now(),
            &self.config.workdir_root,
            &taken,
        );
        let session = PortableSession {
            meta,
            turns: Vec::new(),
            active_leaf: None,
            draft: None,
        };
        self.insert_portable(&session)?;
        self.outbox
            .emit(events::SESSION_CREATED, &id, json!({ "session": id }));
        Ok(session.meta)
    }

    fn adopt_session(&self, session: PortableSession) -> Result<SessionMeta, SessionError> {
        session.validate()?;
        let mut session = session;
        let meta = &mut session.meta;
        match self.exists(&meta.id) {
            Ok(()) => {
                return Err(SessionError::AlreadyExists {
                    id: meta.id.clone(),
                });
            }
            Err(SessionError::NotFound { .. }) => {}
            Err(other) => return Err(other),
        }
        if meta.title.trim().is_empty() {
            DEFAULT_TITLE.clone_into(&mut meta.title);
        }
        meta.tags = normalize_tags(&meta.tags);
        // Plik bez wpisu w katalogu to sierota (np. po przerwanym usuwaniu) — nieczytelna bez klucza.
        lib_sqlstore::remove_database(&self.session_path(&meta.id))
            .map_err(SessionError::storage)?;
        self.insert_portable(&session)?;
        let id = session.meta.id.clone();
        let payload = json!({ "session": id, "imported": true, "turns": session.turns.len() });
        self.outbox.emit(events::SESSION_CREATED, &id, payload);
        Ok(session.meta)
    }

    fn session(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        self.load_meta(id)
    }

    fn update_session(
        &self,
        id: &SessionId,
        patch: SessionPatch,
    ) -> Result<SessionMeta, SessionError> {
        let (before, after) = self.modify(id, |m| m.apply(patch, Utc::now()))?;
        self.outbox
            .emit(events::SESSION_UPDATED, id, json!({ "session": id }));
        if before.archived != after.archived {
            let payload = json!({ "session": id, "archived": after.archived });
            self.outbox.emit(events::SESSION_ARCHIVED, id, payload);
        }
        Ok(after)
    }

    fn mark_tainted(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        let (before, after) = self.modify(id, |m| m.tainted = true)?;
        if !before.tainted {
            self.outbox
                .emit(events::SESSION_TAINTED, id, json!({ "session": id }));
        }
        Ok(after)
    }

    fn list_sessions(&self, query: &SessionQuery) -> Result<Vec<SessionSummary>, SessionError> {
        Ok(apply_query(self.all_summaries()?, query))
    }

    fn set_activity(&self, id: &SessionId, active: bool) -> Result<(), SessionError> {
        self.exists(id)?;
        let mut set = lock(&self.active);
        if active {
            set.insert(id.clone());
        } else {
            set.remove(id);
        }
        Ok(())
    }

    fn mark_read(&self, id: &SessionId) -> Result<(), SessionError> {
        let changed = self
            .index
            .with(|c| {
                c.execute(
                    "UPDATE sessions SET unread = 0 WHERE id = ?1",
                    [id.as_str()],
                )
            })
            .map_err(db_err)?;
        if changed == 0 {
            return Err(SessionError::NotFound { id: id.clone() });
        }
        Ok(())
    }

    fn trash_session(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        let (_, after) = self.modify(id, |m| {
            m.trashed = true;
            m.updated_at = Utc::now().max(m.updated_at);
        })?;
        self.outbox
            .emit(events::SESSION_TRASHED, id, json!({ "session": id }));
        Ok(after)
    }

    fn restore_session(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        let (_, after) = self.modify(id, |m| {
            m.trashed = false;
            m.updated_at = Utc::now().max(m.updated_at);
        })?;
        self.outbox
            .emit(events::SESSION_RESTORED, id, json!({ "session": id }));
        Ok(after)
    }

    fn delete_session(&self, id: &SessionId) -> Result<DeleteReport, SessionError> {
        self.exists(id)?;
        // 1. Klucz najpierw: od tej chwili plik jest nieczytelny (crypto-shredding).
        let key_deleted = self.vault.delete(&session_key_name(id))?;
        // 2. Zamknięcie połączenia i usunięcie plików (`-wal`/`-shm` też).
        let db = lock(&self.open).remove(id);
        if let Some(db) = db.and_then(|db| Arc::try_unwrap(db).ok()) {
            let _ = db.close();
        }
        let files_removed = lib_sqlstore::remove_database(&self.session_path(id))
            .map(|v| v.len())
            .unwrap_or_default();
        // 3. Wpis katalogu. Pliki, których nie dało się usunąć (np. otwarte w innym procesie),
        //    sprząta `sweep_orphans` przy następnym starcie — i tak są nieczytelne.
        self.index
            .with(|c| c.execute("DELETE FROM sessions WHERE id = ?1", [id.as_str()]))
            .map_err(db_err)?;
        lock(&self.active).remove(id);
        self.outbox
            .emit(events::SESSION_DELETED, id, json!({ "session": id }));
        Ok(DeleteReport {
            key_deleted,
            files_removed,
        })
    }
}
