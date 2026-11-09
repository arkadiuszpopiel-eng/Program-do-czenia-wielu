//! Obsada w tej samej szyfrowanej bazie i pod tym samym kluczem co rozmowa.

use std::collections::BTreeMap;

use lib_sqlstore::rusqlite::OptionalExtension;
use personas_contract::{Cast, CastStore};
use sessions_contract::SessionId;

use crate::AppStore;

pub(crate) const MIGRATION: &str = "
CREATE TABLE app_casts(seq INTEGER PRIMARY KEY AUTOINCREMENT, body TEXT NOT NULL);
CREATE TRIGGER app_casts_ro_u BEFORE UPDATE ON app_casts
    BEGIN SELECT RAISE(ABORT, 'app_casts: append-only'); END;
CREATE TRIGGER app_casts_ro_d BEFORE DELETE ON app_casts
    BEGIN SELECT RAISE(ABORT, 'app_casts: append-only'); END;";

impl CastStore for AppStore {
    fn load(&self) -> Result<BTreeMap<SessionId, Cast>, String> {
        let mut casts = BTreeMap::new();
        for session in self.provider.session_ids().map_err(|e| e.to_string())? {
            let body = self.with(&session, |c| {
                c.query_row(
                    "SELECT body FROM app_casts ORDER BY seq DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()
            });
            self.provider.release_idle(&session);
            let body: Option<String> = body.map_err(|e| e.to_string())?;
            if let Some(body) = body {
                casts.insert(
                    session,
                    serde_json::from_str(&body).map_err(|e| e.to_string())?,
                );
            }
        }
        Ok(casts)
    }

    fn save(&self, session: &SessionId, cast: &Cast) -> Result<(), String> {
        let body = serde_json::to_string(cast).map_err(|e| e.to_string())?;
        self.with(session, |c| {
            c.execute("INSERT INTO app_casts(body) VALUES (?1)", [body])
                .map(|_| ())
        })
        .map_err(|e| e.to_string())
    }
}
