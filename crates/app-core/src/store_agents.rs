//! Tabele agentek w szyfrowanej bazie sesji (migracja `0002` przestrzeni `app-core`): katalog
//! roboczy sesji, nagłówki i kroki przebiegów (Replay po restarcie), cofnięte kroki. Tylko
//! dopisywanie — ostatni wpis wygrywa.

use std::collections::{BTreeMap, BTreeSet};

use lib_sqlstore::rusqlite::{OptionalExtension, params};
use sessions_contract::SessionId;

use crate::dto::{AgentRun, AgentRunDetail, ReplayStep};
use crate::error::AppError;
use crate::store::AppStore;

/// Migracja `0002`.
pub(crate) const MIGRATION_0002: &str = "
CREATE TABLE app_workdir(seq INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT, at INTEGER NOT NULL);
CREATE TABLE app_agent_runs(seq INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL, body TEXT NOT NULL);
CREATE TABLE app_agent_steps(seq INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL, step_id TEXT NOT NULL, body TEXT NOT NULL);
CREATE TABLE app_undone(seq INTEGER PRIMARY KEY AUTOINCREMENT,
    token TEXT NOT NULL, at INTEGER NOT NULL);
CREATE TRIGGER app_workdir_ro_u BEFORE UPDATE ON app_workdir
    BEGIN SELECT RAISE(ABORT, 'app_workdir: append-only'); END;
CREATE TRIGGER app_workdir_ro_d BEFORE DELETE ON app_workdir
    BEGIN SELECT RAISE(ABORT, 'app_workdir: append-only'); END;
CREATE TRIGGER app_agent_runs_ro_u BEFORE UPDATE ON app_agent_runs
    BEGIN SELECT RAISE(ABORT, 'app_agent_runs: append-only'); END;
CREATE TRIGGER app_agent_runs_ro_d BEFORE DELETE ON app_agent_runs
    BEGIN SELECT RAISE(ABORT, 'app_agent_runs: append-only'); END;
CREATE TRIGGER app_agent_steps_ro_u BEFORE UPDATE ON app_agent_steps
    BEGIN SELECT RAISE(ABORT, 'app_agent_steps: append-only'); END;
CREATE TRIGGER app_agent_steps_ro_d BEFORE DELETE ON app_agent_steps
    BEGIN SELECT RAISE(ABORT, 'app_agent_steps: append-only'); END;
CREATE TRIGGER app_undone_ro_u BEFORE UPDATE ON app_undone
    BEGIN SELECT RAISE(ABORT, 'app_undone: append-only'); END;
CREATE TRIGGER app_undone_ro_d BEFORE DELETE ON app_undone
    BEGIN SELECT RAISE(ABORT, 'app_undone: append-only'); END;";

fn storage(e: impl std::fmt::Display) -> AppError {
    AppError::storage(e)
}

impl AppStore {
    /// Ustawia katalog roboczy sesji (`None` = agentki bez narzędzi).
    pub fn set_workdir(&self, session: &SessionId, path: Option<&str>) -> Result<(), AppError> {
        self.with(session, |c| {
            c.execute(
                "INSERT INTO app_workdir(path, at) VALUES (?1, ?2)",
                params![path, lib_sqlstore::unix_millis()],
            )
            .map(|_| ())
        })
    }

    /// Katalog roboczy sesji.
    pub fn workdir(&self, session: &SessionId) -> Result<Option<String>, AppError> {
        let row: Option<Option<String>> = self.with(session, |c| {
            c.query_row(
                "SELECT path FROM app_workdir ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
        })?;
        Ok(row.flatten())
    }

    /// Dopisuje nagłówek przebiegu.
    pub fn push_run(&self, session: &SessionId, run: &AgentRun) -> Result<(), AppError> {
        let body = serde_json::to_string(run).map_err(storage)?;
        self.with(session, |c| {
            c.execute(
                "INSERT INTO app_agent_runs(run_id, body) VALUES (?1, ?2)",
                params![run.id, body],
            )
            .map(|_| ())
        })
    }

    /// Dopisuje krok przebiegu.
    pub fn push_step(
        &self,
        session: &SessionId,
        run_id: &str,
        step: &ReplayStep,
    ) -> Result<(), AppError> {
        let body = serde_json::to_string(step).map_err(storage)?;
        self.with(session, |c| {
            c.execute(
                "INSERT INTO app_agent_steps(run_id, step_id, body) VALUES (?1, ?2, ?3)",
                params![run_id, step.id, body],
            )
            .map(|_| ())
        })
    }

    /// Zapamiętuje cofnięty krok (token DTO).
    pub fn push_undone(&self, session: &SessionId, token: &str) -> Result<(), AppError> {
        self.with(session, |c| {
            c.execute(
                "INSERT INTO app_undone(token, at) VALUES (?1, ?2)",
                params![token, lib_sqlstore::unix_millis()],
            )
            .map(|_| ())
        })
    }

    /// Cofnięte kroki sesji.
    pub fn undone(&self, session: &SessionId) -> Result<BTreeSet<String>, AppError> {
        let rows: Vec<String> = self.with(session, |c| {
            let mut stmt = c.prepare("SELECT token FROM app_undone")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect()
        })?;
        Ok(rows.into_iter().collect())
    }

    /// Przebiegi sesji z krokami (ostatnia wersja każdego wpisu), od najstarszego.
    pub fn runs(&self, session: &SessionId) -> Result<Vec<AgentRunDetail>, AppError> {
        let heads: Vec<(String, String)> = self.with(session, |c| {
            let mut stmt = c.prepare("SELECT run_id, body FROM app_agent_runs ORDER BY seq")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })?;
        let steps: Vec<(String, String, String)> = self.with(session, |c| {
            let mut stmt =
                c.prepare("SELECT run_id, step_id, body FROM app_agent_steps ORDER BY seq")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect()
        })?;
        let undone = self.undone(session)?;
        let mut order: Vec<String> = Vec::new();
        let mut runs: BTreeMap<String, AgentRun> = BTreeMap::new();
        for (id, body) in heads {
            let Ok(run) = serde_json::from_str::<AgentRun>(&body) else {
                continue;
            };
            if !runs.contains_key(&id) {
                order.push(id.clone());
            }
            runs.insert(id, run);
        }
        let mut by_run: BTreeMap<String, BTreeMap<String, ReplayStep>> = BTreeMap::new();
        for (run, step_id, body) in steps {
            if let Ok(mut step) = serde_json::from_str::<ReplayStep>(&body) {
                step.undone |= step.undo_token.as_ref().is_some_and(|t| undone.contains(t));
                by_run.entry(run).or_default().insert(step_id, step);
            }
        }
        Ok(order
            .into_iter()
            .filter_map(|id| {
                let run = runs.remove(&id)?;
                let mut steps: Vec<ReplayStep> = by_run
                    .remove(&id)
                    .unwrap_or_default()
                    .into_values()
                    .collect();
                steps.sort_by(|a, b| (a.n, a.at_ms).cmp(&(b.n, b.at_ms)));
                Some(AgentRunDetail { run, steps })
            })
            .collect())
    }
}
