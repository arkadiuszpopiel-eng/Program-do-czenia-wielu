//! Tabele `app-core` w szyfrowanej bazie sesji (`SessionDbProvider`, przestrzeń migracji
//! `app-core`): fakty o turach, których nie ma w kontrakcie `sessions` (stan, błąd, agentka,
//! adresatka, kontynuacja, zużycie w PLN), log stanów (kolejka offline), oceny i oś czasu v0.
//! Wszystko **wyłącznie dopisywane** (wyzwalacze blokują UPDATE/DELETE); usunięcie sesji
//! (crypto-shredding) usuwa je razem z bazą.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use lib_sqlstore::rusqlite::{Connection, OptionalExtension, params};
use lib_sqlstore::{Db, migrate};
use serde::{Deserialize, Serialize};
use sessions_contract::{SessionDbProvider, SessionId, TurnId};

use crate::dto::{
    ApprovalPending, Rating, TimelineEvent, ToolStep, TurnError, TurnStatus, TurnUsage,
};
use crate::error::AppError;

const NAMESPACE: &str = "app-core";

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001",
        "CREATE TABLE app_turn_meta(turn_id INTEGER PRIMARY KEY, body TEXT NOT NULL);
     CREATE TABLE app_turn_status(seq INTEGER PRIMARY KEY AUTOINCREMENT,
         turn_id INTEGER NOT NULL, status TEXT NOT NULL, at INTEGER NOT NULL);
     CREATE TABLE app_ratings(seq INTEGER PRIMARY KEY AUTOINCREMENT,
         turn_id INTEGER NOT NULL, rating TEXT, at INTEGER NOT NULL);
     CREATE TABLE app_timeline(seq INTEGER PRIMARY KEY AUTOINCREMENT, body TEXT NOT NULL);
     CREATE TRIGGER app_turn_meta_ro_u BEFORE UPDATE ON app_turn_meta
         BEGIN SELECT RAISE(ABORT, 'app_turn_meta: append-only'); END;
     CREATE TRIGGER app_turn_meta_ro_d BEFORE DELETE ON app_turn_meta
         BEGIN SELECT RAISE(ABORT, 'app_turn_meta: append-only'); END;
     CREATE TRIGGER app_turn_status_ro_u BEFORE UPDATE ON app_turn_status
         BEGIN SELECT RAISE(ABORT, 'app_turn_status: append-only'); END;
     CREATE TRIGGER app_ratings_ro_u BEFORE UPDATE ON app_ratings
         BEGIN SELECT RAISE(ABORT, 'app_ratings: append-only'); END;
     CREATE TRIGGER app_timeline_ro_u BEFORE UPDATE ON app_timeline
         BEGIN SELECT RAISE(ABORT, 'app_timeline: append-only'); END;",
    ),
    ("0002", crate::store_agents::MIGRATION_0002),
];

/// Fakty o turze zapisywane raz, razem z turą.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnMeta {
    /// Stan końcowy (`complete` / `cancelled` / `error`; tury użytkownika — `complete`).
    pub status: Option<TurnStatus>,
    /// Agentka (także dla tur-błędów zapisanych jako komunikat systemowy).
    pub agent: Option<String>,
    /// Rola agentki w chwili odpowiedzi.
    pub role_id: Option<String>,
    /// Adresatka wiadomości użytkownika.
    pub addressed_to: Option<String>,
    /// Kontynuowana tura.
    pub continues: Option<u64>,
    /// Odpowiedź ucięta (`max_tokens`).
    pub truncated: bool,
    /// Czas myślenia.
    pub thinking_ms: Option<u64>,
    /// Zużycie i koszt w PLN.
    pub usage: Option<TurnUsage>,
    /// Błąd.
    pub error: Option<TurnError>,
    /// Kroki narzędzi (tura agentki z narzędziami).
    #[serde(default)]
    pub tools: Vec<ToolStep>,
    /// Ostatnia karta „czeka na zatwierdzenie".
    #[serde(default)]
    pub approval: Option<ApprovalPending>,
}

/// Dostęp do tabel `app-core` w bazach sesji.
pub struct AppStore {
    provider: Arc<dyn SessionDbProvider>,
    migrated: Mutex<HashSet<SessionId>>,
}

fn storage(e: impl std::fmt::Display) -> AppError {
    AppError::storage(e)
}

fn status_name(status: TurnStatus) -> &'static str {
    match status {
        TurnStatus::Queued => "queued",
        TurnStatus::Streaming => "streaming",
        TurnStatus::Complete => "complete",
        TurnStatus::Cancelled => "cancelled",
        TurnStatus::Error => "error",
    }
}

fn parse_status(s: &str) -> Option<TurnStatus> {
    [
        TurnStatus::Queued,
        TurnStatus::Streaming,
        TurnStatus::Complete,
        TurnStatus::Cancelled,
        TurnStatus::Error,
    ]
    .into_iter()
    .find(|t| status_name(*t) == s)
}

pub(crate) fn to_i64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

impl AppStore {
    /// Magazyn na bazach sesji.
    pub fn new(provider: Arc<dyn SessionDbProvider>) -> Self {
        Self {
            provider,
            migrated: Mutex::new(HashSet::new()),
        }
    }

    fn db(&self, session: &SessionId) -> Result<Arc<Db>, AppError> {
        let db = self.provider.session_db(session).map_err(AppError::from)?;
        let fresh = !self
            .migrated
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(session);
        if fresh {
            db.with(|c| migrate(c, NAMESPACE, MIGRATIONS).map(|_| ()))
                .map_err(storage)?;
            self.migrated
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(session.clone());
        }
        Ok(db)
    }

    pub(crate) fn with<R>(
        &self,
        session: &SessionId,
        f: impl FnOnce(&mut Connection) -> Result<R, lib_sqlstore::rusqlite::Error>,
    ) -> Result<R, AppError> {
        self.db(session)?.with(f).map_err(storage)
    }

    /// Zapomina stan migracji sesji (po usunięciu — przyszła sesja o tym id dostanie nową bazę).
    pub fn forget(&self, session: &SessionId) {
        self.migrated
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(session);
    }

    /// Zapisuje fakty o turze (raz).
    pub fn put_meta(
        &self,
        session: &SessionId,
        turn: TurnId,
        meta: &TurnMeta,
    ) -> Result<(), AppError> {
        let body = serde_json::to_string(meta).map_err(storage)?;
        self.with(session, |c| {
            c.execute(
                "INSERT INTO app_turn_meta(turn_id, body) VALUES (?1, ?2)",
                params![to_i64(turn.0), body],
            )
            .map(|_| ())
        })
    }

    /// Wszystkie fakty o turach sesji.
    pub fn metas(&self, session: &SessionId) -> Result<BTreeMap<u64, TurnMeta>, AppError> {
        let rows: Vec<(i64, String)> = self.with(session, |c| {
            let mut stmt = c.prepare("SELECT turn_id, body FROM app_turn_meta")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, body)| {
                let meta = serde_json::from_str(&body).ok()?;
                Some((u64::try_from(id).ok()?, meta))
            })
            .collect())
    }

    /// Fakty o jednej turze.
    pub fn meta(&self, session: &SessionId, turn: TurnId) -> Result<Option<TurnMeta>, AppError> {
        let body: Option<String> = self.with(session, |c| {
            c.query_row(
                "SELECT body FROM app_turn_meta WHERE turn_id = ?1",
                params![to_i64(turn.0)],
                |r| r.get(0),
            )
            .optional()
        })?;
        Ok(body.and_then(|b| serde_json::from_str(&b).ok()))
    }

    /// Dopisuje zmianę stanu tury (np. kolejka offline → wysłana).
    pub fn push_status(
        &self,
        session: &SessionId,
        turn: TurnId,
        status: TurnStatus,
    ) -> Result<(), AppError> {
        self.with(session, |c| {
            c.execute(
                "INSERT INTO app_turn_status(turn_id, status, at) VALUES (?1, ?2, ?3)",
                params![
                    to_i64(turn.0),
                    status_name(status),
                    lib_sqlstore::unix_millis()
                ],
            )
            .map(|_| ())
        })
    }

    /// Ostatni stan per tura (z logu).
    pub fn statuses(&self, session: &SessionId) -> Result<BTreeMap<u64, TurnStatus>, AppError> {
        let rows: Vec<(i64, String)> = self.with(session, |c| {
            let mut stmt = c.prepare("SELECT turn_id, status FROM app_turn_status ORDER BY seq")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, s)| Some((u64::try_from(id).ok()?, parse_status(&s)?)))
            .collect())
    }

    /// Dopisuje ocenę (ostatnia wygrywa; `None` = cofnięcie oceny).
    pub fn push_rating(
        &self,
        session: &SessionId,
        turn: TurnId,
        rating: Option<Rating>,
    ) -> Result<(), AppError> {
        let value = rating.map(|r| match r {
            Rating::Up => "up",
            Rating::Down => "down",
        });
        self.with(session, |c| {
            c.execute(
                "INSERT INTO app_ratings(turn_id, rating, at) VALUES (?1, ?2, ?3)",
                params![to_i64(turn.0), value, lib_sqlstore::unix_millis()],
            )
            .map(|_| ())
        })
    }

    /// Aktualne oceny per tura.
    pub fn ratings(&self, session: &SessionId) -> Result<BTreeMap<u64, Rating>, AppError> {
        let rows: Vec<(i64, Option<String>)> = self.with(session, |c| {
            let mut stmt = c.prepare("SELECT turn_id, rating FROM app_ratings ORDER BY seq")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })?;
        let mut out = BTreeMap::new();
        for (id, rating) in rows {
            let Ok(id) = u64::try_from(id) else { continue };
            match rating.as_deref() {
                Some("up") => out.insert(id, Rating::Up),
                Some("down") => out.insert(id, Rating::Down),
                _ => out.remove(&id),
            };
        }
        Ok(out)
    }

    /// Dopisuje zdarzenie osi czasu; zwraca numer kolejny.
    pub fn push_timeline(
        &self,
        session: &SessionId,
        event: &TimelineEvent,
    ) -> Result<i64, AppError> {
        let body = serde_json::to_string(event).map_err(storage)?;
        self.with(session, |c| {
            c.execute("INSERT INTO app_timeline(body) VALUES (?1)", params![body])?;
            Ok(c.last_insert_rowid())
        })
    }

    /// Zdarzenia osi czasu sesji (rosnąco).
    pub fn timeline(&self, session: &SessionId) -> Result<Vec<TimelineEvent>, AppError> {
        let rows: Vec<String> = self.with(session, |c| {
            let mut stmt = c.prepare("SELECT body FROM app_timeline ORDER BY seq")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect()
        })?;
        Ok(rows
            .iter()
            .filter_map(|b| serde_json::from_str(b).ok())
            .collect())
    }
}
