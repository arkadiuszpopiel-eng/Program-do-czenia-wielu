//! Atomowy stan wznowienia w szyfrowanej bazie sesji, niezależny od append-only historii.
use crate::AppStore;
use app_api::AppError;
use lib_sqlstore::rusqlite::{OptionalExtension, params};
use serde_json::Value;
use sessions_contract::SessionId;

pub(crate) const MIGRATION: &str = "CREATE TABLE app_work_state(key TEXT PRIMARY KEY, kind TEXT NOT NULL, owner_turn TEXT, body TEXT NOT NULL, updated INTEGER NOT NULL); CREATE INDEX app_work_turn ON app_work_state(owner_turn);";

impl AppStore {
    /// Zapis najnowszego punktu wznowienia; commit jest atomowy.
    pub fn save_work(
        &self,
        session: &SessionId,
        key: &str,
        kind: &str,
        turn: Option<&str>,
        body: &Value,
    ) -> Result<(), AppError> {
        let body = serde_json::to_string(body).map_err(AppError::storage)?;
        self.with(session,|c|c.execute("INSERT INTO app_work_state(key,kind,owner_turn,body,updated) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(key) DO UPDATE SET kind=excluded.kind,owner_turn=excluded.owner_turn,body=excluded.body,updated=excluded.updated",params![key,kind,turn,body,lib_sqlstore::unix_millis()]).map(|_|()))
    }
    /// Odczyt punktu wznowienia.
    pub fn work(&self, session: &SessionId, key: &str) -> Result<Option<Value>, AppError> {
        let body: Option<String> = self.with(session, |c| {
            c.query_row("SELECT body FROM app_work_state WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()
        })?;
        body.map(|b| serde_json::from_str(&b).map_err(AppError::storage))
            .transpose()
    }
    /// Punkty związane z turą (od najnowszego).
    pub fn work_for_turn(
        &self,
        session: &SessionId,
        turn: &str,
    ) -> Result<Vec<(String, String, Value)>, AppError> {
        let rows:Vec<(String,String,String)>=self.with(session,|c|c.prepare("SELECT key,kind,body FROM app_work_state WHERE owner_turn=?1 ORDER BY updated DESC")?.query_map([turn],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect())?;
        rows.into_iter()
            .map(|(key, kind, body)| {
                Ok((
                    key,
                    kind,
                    serde_json::from_str(&body).map_err(AppError::storage)?,
                ))
            })
            .collect()
    }
    /// Identyfikatory punktów określonego typu w tej sesji.
    pub fn work_keys(&self, session: &SessionId, kind: &str) -> Result<Vec<String>, AppError> {
        self.with(session, |c| {
            c.prepare("SELECT key FROM app_work_state WHERE kind=?1 ORDER BY key")?
                .query_map([kind], |r| r.get(0))?
                .collect()
        })
    }
}
