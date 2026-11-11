//! Odczyt minimalnych pól Replay. Nie przekazujemy celów, argumentów, etykiet i wyjść.
use std::collections::{BTreeMap, BTreeSet};

use app_api::{
    dto::{ReplayKind, ReplayStatus},
    ids,
};
use lib_sqlstore::rusqlite::{Connection, Result};
use serde::Deserialize;
use sessions_contract::SessionId;

#[derive(Deserialize)]
struct Head {
    id: String,
    session_id: String,
    agent: String,
    turn_id: Option<String>,
    bridge: Option<String>,
}

#[derive(Deserialize)]
struct Step {
    id: String,
    n: u32,
    kind: ReplayKind,
    tool: Option<String>,
    status: ReplayStatus,
}

pub(crate) struct ReplayRecord {
    pub owner_turn: String,
    pub agent: String,
    pub bridge: bool,
    pub native_checkpoint: bool,
    pub tool: Option<String>,
    pub status: ReplayStatus,
}

/// Brak wpisu = tylko metadane tury. `Some(None)` = nieczytelny/niespójny Replay.
pub(crate) fn read(
    db: &Connection,
    session: &SessionId,
    selected_turns: &BTreeSet<String>,
    selected_steps: &BTreeSet<String>,
) -> Result<BTreeMap<String, Option<ReplayRecord>>> {
    let runs: BTreeSet<_> = selected_steps
        .iter()
        .filter_map(|id| {
            let (sid, run, _) = ids::parse_step(id).ok()?;
            (sid == *session).then(|| ids::run_dto(session, &run))
        })
        .collect();
    if runs.is_empty() {
        return Ok(BTreeMap::new());
    }
    // Same kolumny pochodzenia; treść checkpointu (argumenty/wyjścia) nie jest czytana.
    let mut native = BTreeSet::new();
    let mut query =
        db.prepare("SELECT key, owner_turn FROM app_work_state WHERE kind='runtime'")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        let key: String = row.get(0)?;
        let owner: Option<String> = row.get(1)?;
        let run = ids::run_dto(session, &key);
        if runs.contains(&run)
            && let Some(owner) = owner.filter(|t| selected_turns.contains(t))
        {
            native.insert((run, owner));
        }
    }
    drop(rows);
    drop(query);
    let mut heads = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut query = db.prepare("SELECT run_id, body FROM app_agent_runs ORDER BY seq DESC")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        let run: String = row.get(0)?;
        if !runs.contains(&run) || !seen.insert(run.clone()) {
            continue;
        }
        let body: String = row.get(1)?;
        let head = serde_json::from_str::<Head>(&body).ok().filter(|h| {
            h.id == run
                && h.session_id == session.to_string()
                && h.turn_id
                    .as_ref()
                    .is_some_and(|turn| selected_turns.contains(turn))
        });
        heads.insert(run, head);
        if seen.len() == runs.len() {
            break;
        }
    }
    drop(rows);
    drop(query);
    let mut out = BTreeMap::new();
    let mut query =
        db.prepare("SELECT run_id, step_id, body FROM app_agent_steps ORDER BY seq DESC")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        let run: String = row.get(0)?;
        let id: String = row.get(1)?;
        if !selected_steps.contains(&id) || out.contains_key(&id) {
            continue;
        }
        // Kolumna run_id też musi zgadzać się z identyfikatorem kroku; nie ufamy samej nazwie.
        let ordinal = ids::parse_step(&id).ok().and_then(|(sid, raw, n)| {
            (sid == *session && ids::run_dto(session, &raw) == run).then_some(n)
        });
        let value = if let Some(ordinal) = ordinal {
            if let Some(head) = heads.get(&run).and_then(Option::as_ref) {
                let body: String = row.get(2)?;
                serde_json::from_str::<Step>(&body)
                    .ok()
                    .filter(|step| {
                        step.id == id
                            && step.n == ordinal
                            && step.kind == ReplayKind::Tool
                            && step.tool.as_ref().is_some_and(|n| !n.is_empty())
                    })
                    .and_then(|step| {
                        Some(ReplayRecord {
                            owner_turn: head.turn_id.clone()?,
                            agent: head.agent.clone(),
                            bridge: head.bridge.is_some(),
                            native_checkpoint: native
                                .contains(&(run.clone(), head.turn_id.clone()?)),
                            tool: step.tool,
                            status: step.status,
                        })
                    })
            } else {
                None
            }
        } else {
            None
        };
        out.insert(id, value);
        if out.len() == selected_steps.len() {
            break;
        }
    }
    Ok(out)
}
