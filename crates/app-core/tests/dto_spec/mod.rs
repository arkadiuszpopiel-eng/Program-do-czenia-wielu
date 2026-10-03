//! Specyfikacje komend F5–F7 dla `dto_roundtrip.rs` (projekt sesji, pamięć, zadania, wyzwalacze,
//! Marszałek, mosty CLI): typy argumentów (camelCase jak w `TauriAlfaClient`) i wyniku.

use app_core::dto::*;

use super::{Check, roundtrip};

mod computer;
mod updates;

type Spec = (Vec<(&'static str, Check)>, Check);

fn memory(command: &str) -> Option<Spec> {
    let s: Check = roundtrip::<String>;
    let item: Check = roundtrip::<MemoryItem>;
    let target: Check = roundtrip::<MemoryForgetTarget>;
    Some(match command {
        "memory_status" => (vec![], roundtrip::<MemoryStatus>),
        "memory_scopes" => (vec![], roundtrip::<Vec<MemoryScopeInfo>>),
        "memory_inspect" => (
            vec![("query", roundtrip::<MemoryQuery>)],
            roundtrip::<MemoryPage>,
        ),
        "memory_explain" => (vec![("entryId", s)], roundtrip::<MemoryExplanation>),
        "memory_edit" => (
            vec![("entryId", s), ("edit", roundtrip::<MemoryEdit>)],
            item,
        ),
        "memory_set_pinned" => (vec![("entryId", s), ("pinned", roundtrip::<bool>)], item),
        "memory_approve" => (vec![("entryId", s)], item),
        "memory_promote" => (
            vec![("entryId", s), ("to", roundtrip::<MemoryScopeRef>)],
            item,
        ),
        "memory_forget_preview" => (vec![("target", target)], roundtrip::<MemoryForgetPreview>),
        "memory_forget" => (vec![("target", target)], roundtrip::<MemoryForgetReport>),
        "memory_journal" => (vec![("scope", s)], roundtrip::<Vec<MemoryJournalEntry>>),
        "memory_undo" => (
            vec![("scope", s), ("changeId", s)],
            roundtrip::<MemoryUndoResult>,
        ),
        "memory_consolidate_now" => (vec![], roundtrip::<ConsolidationReport>),
        _ => return None,
    })
}

fn tasks(command: &str) -> Option<Spec> {
    let s: Check = roundtrip::<String>;
    let os: Check = roundtrip::<Option<String>>;
    let unit: Check = roundtrip::<()>;
    let task: Check = roundtrip::<TaskInfo>;
    Some(match command {
        "tasks_list" => (vec![], roundtrip::<Vec<TaskInfo>>),
        "tasks_create" => (vec![("input", roundtrip::<NewTaskInput>)], task),
        "tasks_cancel" => (vec![("taskId", s)], roundtrip::<Vec<String>>),
        "tasks_retry" => (vec![("taskId", s)], task),
        "tasks_steer" => (vec![("taskId", s), ("text", s)], unit),
        "tasks_pause" | "tasks_resume" => (vec![("taskId", s)], unit),
        "triggers_list" => (vec![], roundtrip::<Vec<TriggerInfo>>),
        "triggers_create" => (
            vec![("draft", roundtrip::<TriggerDraft>)],
            roundtrip::<TriggerInfo>,
        ),
        "triggers_remove" => (vec![("triggerId", s)], unit),
        "triggers_set_enabled" => (vec![("triggerId", s), ("enabled", roundtrip::<bool>)], unit),
        "triggers_fire_now" => (vec![("triggerId", s)], roundtrip::<TriggerRunInfo>),
        "triggers_log" => (vec![("triggerId", os)], roundtrip::<Vec<TriggerRunInfo>>),
        "triggers_preview_cron" => (vec![("expr", s)], roundtrip::<CronPreview>),
        "marshal_state" => (vec![], roundtrip::<MarshalState>),
        "marshal_propose" => (
            vec![
                ("text", s),
                ("drafts", roundtrip::<Option<Vec<serde_json::Value>>>),
            ],
            roundtrip::<MarshalProposalInfo>,
        ),
        "marshal_approve" => (
            vec![("proposalId", roundtrip::<u64>)],
            roundtrip::<Vec<MarshalRuleInfo>>,
        ),
        "marshal_reject" => (vec![("proposalId", roundtrip::<u64>)], unit),
        "marshal_revoke" => (vec![("ruleId", s)], unit),
        "marshal_report" => (vec![], roundtrip::<MarshalReport>),
        _ => return None,
    })
}

/// Specyfikacja komendy z fal F5–F7 (`None` — komenda spoza tej części).
pub fn spec(command: &str) -> Option<Spec> {
    let s: Check = roundtrip::<String>;
    let os: Check = roundtrip::<Option<String>>;
    let card: Check = roundtrip::<BridgeCard>;
    let found: Spec = match command {
        "sessions_set_project" => (vec![("sessionId", s), ("project", os)], roundtrip::<()>),
        "bridges_list" => (
            vec![("refresh", roundtrip::<bool>)],
            roundtrip::<Vec<BridgeCard>>,
        ),
        "bridges_set_enabled" => (vec![("routeId", s), ("enabled", roundtrip::<bool>)], card),
        "bridges_set_schedule" => (vec![("bridge", s), ("perDay", roundtrip::<u32>)], card),
        "bridges_pin" => (vec![("bridge", s), ("version", os)], card),
        "bridges_open_login" => (vec![("bridge", s)], roundtrip::<BridgeLogin>),
        other => {
            return memory(other)
                .or_else(|| tasks(other))
                .or_else(|| computer::spec(other))
                .or_else(|| updates::spec(other));
        }
    };
    Some(found)
}
