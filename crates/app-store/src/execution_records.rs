//! Ograniczone fakty zapisanych stanów wybranej gałęzi, niezależne od wypowiedzi modelu.
use std::collections::{BTreeMap, BTreeSet};

use app_api::{
    AppError,
    dto::{ReplayStatus, ToolStatus},
    ids,
};
use lib_sqlstore::rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize, de::IgnoredAny};
use sessions_contract::{SessionId, TurnId};

use crate::{
    AppStore,
    execution_replay::{self, ReplayRecord},
};

const MAX_RECORDS: usize = 4;
const MAX_CONTEXT_BYTES: usize = 1536;

#[derive(Deserialize)]
struct RecordMeta {
    agent: Option<String>,
    tools: Option<Vec<RecordStep>>,
}

#[cfg(test)]
#[path = "execution_records_tests.rs"]
mod tests;

#[derive(Deserialize)]
struct RecordStep {
    id: String,
    status: ToolStatus,
    undo_token: Option<String>,
    #[serde(default)]
    undone: bool,
    intent: Option<IgnoredAny>,
}

/// Stan rejestru — nie certyfikat skutków ani zaliczonych testów.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedState {
    /// Zapisano Done; nie jest to dowód aktualnego skutku.
    RecordedDone,
    /// Propozycja czekająca na zgodę.
    NeedsConfirmation,
    /// Zapis błędu, odmowy lub anulowania.
    ErrorOrDenied,
    /// Zapis trwającego kroku.
    RecordedRunning,
    /// Operacja oznaczona jako cofnięta.
    Undone,
    /// Brak spójnego potwierdzenia stanu.
    Unverified,
}

/// Źródło, niezależne od tekstu deklaracji modelu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordOrigin {
    /// Same metadane tury, bez natywnego pochodzenia Replay.
    TurnMetadataOnly,
    /// Zgodne metadane, Replay i powiązanie z checkpointem runtime.
    NativeReplay,
    /// Deklaracja mostu CLI bez potwierdzenia wykonania.
    UnverifiedCli,
    /// Nieczytelny lub sprzeczny najnowszy zapis Replay.
    InconsistentReplay,
}

/// Minimalny wpis; bez ID, ścieżek, argumentów, intencji, etykiet lub wyjścia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionRecord {
    /// Identyfikator persony, jeśli ma bezpieczny format.
    pub agent: Option<String>,
    /// Nazwa z zarejestrowanego manifestu, tylko dla natywnego Replay.
    pub tool: Option<String>,
    /// Oryginalny status z metadanych, także po Undo.
    pub recorded_status: ToolStatus,
    /// Stan po uwzględnieniu pochodzenia, spójności i Undo.
    pub state: RecordedState,
    /// Źródło zapisu; nie jest certyfikatem fizycznego skutku.
    pub origin: RecordOrigin,
}

/// Liczniki rozłącznych aktualnych stanów zapisów po deduplikacji ID kroków.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ExecutionCounts {
    /// Zapisane Done, z wyłączeniem cofniętych i niezweryfikowanych.
    pub recorded_done: u64,
    /// Propozycje wymagające zgody.
    pub needs_confirmation: u64,
    /// Błędy, odmowy lub anulowania.
    pub error_or_denied: u64,
    /// Kroki zapisane jako trwające.
    pub recorded_running: u64,
    /// Cofnięte operacje.
    pub undone: u64,
    /// Zapisy bez spójnego potwierdzenia.
    pub unverified: u64,
}

impl ExecutionCounts {
    fn add(&mut self, state: RecordedState) {
        let n = match state {
            RecordedState::RecordedDone => &mut self.recorded_done,
            RecordedState::NeedsConfirmation => &mut self.needs_confirmation,
            RecordedState::ErrorOrDenied => &mut self.error_or_denied,
            RecordedState::RecordedRunning => &mut self.recorded_running,
            RecordedState::Undone => &mut self.undone,
            RecordedState::Unverified => &mut self.unverified,
        };
        *n = n.saturating_add(1);
    }
}

/// Migawka metadanych i Replay z jednej transakcji odczytu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionRecords {
    /// Stałe oznaczenie danych pochodzących z aplikacji.
    pub source: &'static str,
    /// Wybrana gałąź przed bieżącą turą.
    pub scope: &'static str,
    /// Odczyt kontekstu nie jest akcją wykonaną przez agentkę.
    pub lookup_is_not_agent_action: bool,
    /// Ograniczenia interpretacji zapisów.
    pub limits: &'static str,
    /// Liczniki wszystkich wybranych kroków po deduplikacji.
    pub counts: ExecutionCounts,
    /// Tury z brakującymi lub nieczytelnymi metadanymi.
    pub missing_metadata_turns: u64,
    /// Czy pominięto starsze szczegóły; liczniki pozostają pełne.
    pub records_truncated: bool,
    /// Najnowsze szczegóły mieszczące się w budżecie kontekstu.
    pub records: Vec<ActionRecord>,
}

fn safe_name(name: &str) -> Option<String> {
    (!name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')))
    .then(|| name.to_owned())
}

fn consistent(status: ToolStatus, replay: ReplayStatus) -> bool {
    match status {
        ToolStatus::Done => replay == ReplayStatus::Ok,
        ToolStatus::NeedsConfirmation => replay == ReplayStatus::NeedsConfirmation,
        ToolStatus::Running => matches!(
            replay,
            ReplayStatus::Running | ReplayStatus::WaitingApproval
        ),
        ToolStatus::Error => matches!(
            replay,
            ReplayStatus::Failed | ReplayStatus::Denied | ReplayStatus::Cancelled
        ),
    }
}

fn project(
    step: &RecordStep,
    owner: &str,
    agent: Option<&str>,
    replay: Option<&Option<ReplayRecord>>,
    undone: &BTreeSet<String>,
    known_tools: &BTreeSet<String>,
) -> ActionRecord {
    let valid = replay
        .and_then(Option::as_ref)
        .filter(|r| r.owner_turn == owner);
    let origin = match (replay, valid) {
        (_, Some(r)) if r.bridge => RecordOrigin::UnverifiedCli,
        (_, Some(r)) if !consistent(step.status, r.status) => RecordOrigin::InconsistentReplay,
        (_, Some(r)) if r.native_checkpoint => RecordOrigin::NativeReplay,
        (_, Some(_)) => RecordOrigin::TurnMetadataOnly,
        (Some(_), _) => RecordOrigin::InconsistentReplay,
        _ => RecordOrigin::TurnMetadataOnly,
    };
    let state = if step.id.is_empty()
        || matches!(
            origin,
            RecordOrigin::UnverifiedCli | RecordOrigin::InconsistentReplay
        )
        || (step.status == ToolStatus::Done && step.intent.is_some())
    {
        RecordedState::Unverified
    } else if step.undone || step.undo_token.as_ref().is_some_and(|t| undone.contains(t)) {
        RecordedState::Undone
    } else {
        match step.status {
            ToolStatus::Done => RecordedState::RecordedDone,
            ToolStatus::NeedsConfirmation => RecordedState::NeedsConfirmation,
            ToolStatus::Running => RecordedState::RecordedRunning,
            ToolStatus::Error => RecordedState::ErrorOrDenied,
        }
    };
    ActionRecord {
        agent: match valid {
            Some(record) => safe_name(&record.agent),
            None => agent.and_then(safe_name),
        },
        tool: valid
            .filter(|_| origin == RecordOrigin::NativeReplay)
            .and_then(|r| {
                r.tool
                    .as_deref()
                    .filter(|t| known_tools.contains(*t))
                    .and_then(safe_name)
            }),
        recorded_status: step.status,
        state,
        origin,
    }
}

fn read_metas(
    db: &lib_sqlstore::rusqlite::Connection,
    session: &SessionId,
    selected: &[TurnId],
) -> lib_sqlstore::rusqlite::Result<(Vec<(String, RecordMeta)>, u64)> {
    let mut metas = Vec::new();
    let mut missing = 0_u64;
    let mut seen = BTreeSet::new();
    let mut query = db.prepare("SELECT body FROM app_turn_meta WHERE turn_id=?1")?;
    for id in selected {
        if !seen.insert(id.0) {
            continue;
        }
        let raw = i64::try_from(id.0).ok().filter(|n| *n > 0);
        let Some(raw) = raw else {
            missing = missing.saturating_add(1);
            continue;
        };
        let body: Option<String> = query.query_row([raw], |r| r.get(0)).optional()?;
        let meta = body
            .and_then(|b| serde_json::from_str::<RecordMeta>(&b).ok())
            .filter(|m| m.tools.is_some());
        match meta {
            Some(meta) => metas.push((ids::turn_dto(session, *id), meta)),
            None => missing = missing.saturating_add(1),
        }
    }
    Ok((metas, missing))
}

fn read_undo(
    db: &lib_sqlstore::rusqlite::Connection,
    metas: &[(String, RecordMeta)],
) -> lib_sqlstore::rusqlite::Result<BTreeSet<String>> {
    let wanted: BTreeSet<_> = metas
        .iter()
        .flat_map(|(_, m)| m.tools.iter().flatten())
        .filter_map(|s| s.undo_token.as_deref())
        .collect();
    let mut undone = BTreeSet::new();
    if !wanted.is_empty() {
        let mut query = db.prepare("SELECT token FROM app_undone")?;
        for token in query.query_map([], |r| r.get::<_, String>(0))? {
            let token = token?;
            if wanted.contains(token.as_str()) {
                undone.insert(token);
            }
        }
    }
    Ok(undone)
}

fn summarize(
    metas: &[(String, RecordMeta)],
    missing: u64,
    replay: &BTreeMap<String, Option<ReplayRecord>>,
    undone: &BTreeSet<String>,
    known_tools: &BTreeSet<String>,
) -> ExecutionRecords {
    // Ostatni wpis wygrywa; ID nie opuszczają funkcji. Brakujące ID pozostają osobne.
    let mut records = BTreeMap::new();
    let mut index = 0_usize;
    for (owner, meta) in metas {
        for step in meta.tools.iter().flatten() {
            let key = if step.id.is_empty() {
                (None, index)
            } else {
                (Some(step.id.clone()), 0)
            };
            records.insert(
                key,
                (
                    index,
                    project(
                        step,
                        owner,
                        meta.agent.as_deref(),
                        replay.get(&step.id),
                        undone,
                        known_tools,
                    ),
                ),
            );
            index = index.saturating_add(1);
        }
    }
    let mut ordered: Vec<_> = records.into_values().collect();
    ordered.sort_by_key(|(i, _)| *i);
    let mut counts = ExecutionCounts::default();
    for (_, record) in &ordered {
        counts.add(record.state);
    }
    let count = ordered.len();
    let records = ordered
        .into_iter()
        .rev()
        .take(MAX_RECORDS)
        .map(|(_, r)| r)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    ExecutionRecords {
        source: "application_execution_records",
        scope: "selected_branch_before_current_turn",
        lookup_is_not_agent_action: true,
        limits: "Done jest zapisanym statusem, nie dowodem aktualnego skutku, zaliczonych testów lub przyczyny odmowy. Propozycja nie jest wykonaniem. Undo cofa wcześniejszą operację; brak metadanych oznacza stan nieznany.",
        counts,
        missing_metadata_turns: missing,
        records_truncated: count > MAX_RECORDS,
        records,
    }
}

impl AppStore {
    /// `selected` pochodzi z projekcji gałęzi historii: tylko wcześniejsze tury agentek,
    /// także marker błędu po narzędziu. Pusty wybór nie otwiera bazy.
    /// Nazwy pochodzą z `known_tools` zarejestrowanych manifestów; pozostałe są ukryte.
    pub fn execution_records(
        &self,
        session: &SessionId,
        selected: &[TurnId],
        known_tools: &BTreeSet<String>,
    ) -> Result<Option<ExecutionRecords>, AppError> {
        if selected.is_empty() {
            return Ok(None);
        }
        let mut result = self.with(session, |db| {
            let tx = db.transaction()?;
            let (metas, missing) = read_metas(&tx, session, selected)?;
            let turns = metas.iter().map(|(id, _)| id.clone()).collect();
            let steps = metas
                .iter()
                .flat_map(|(_, m)| m.tools.iter().flatten().map(|s| s.id.clone()))
                .collect();
            let replay = execution_replay::read(&tx, session, &turns, &steps)?;
            let undone = read_undo(&tx, &metas)?;
            let result = summarize(&metas, missing, &replay, &undone, known_tools);
            tx.commit()?;
            Ok(result)
        })?;
        while serde_json::to_vec(&result)
            .map_err(AppError::storage)?
            .len()
            > MAX_CONTEXT_BYTES
        {
            if result.records.is_empty() {
                return Err(AppError::internal("Za duży nagłówek faktów wykonania"));
            }
            result.records.remove(0);
            result.records_truncated = true;
        }
        Ok(Some(result))
    }
}
