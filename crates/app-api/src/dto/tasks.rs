//! DTO panelu Zadania (scheduler: DAG, stan, postęp, steering), Wyzwalaczy (czasowe, zdarzeniowe,
//! ręczne, dziennik uruchomień) i Reguł Marszałka (propozycja → podgląd zawężenia → zatwierdź /
//! cofnij, raport dzienny) — odpowiedniki `types-tasks.ts`.

use serde::{Deserialize, Serialize};

use super::common::{Iso8601, Money};

/// Stan zadania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStateKind {
    Pending,
    Ready,
    Running,
    RetryWait,
    Paused,
    Done,
}

/// Wynik zakończonego zadania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskResultKind {
    Succeeded,
    Failed,
    Cancelled,
    Skipped,
    Expired,
    BudgetExceeded,
    BudgetBlocked,
}

/// Klasa priorytetu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClassKind {
    User,
    Agent,
    Background,
}

/// Pochodzenie zadania (mosty CLI tylko z `user` albo `schedule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOriginKind {
    User,
    Agent,
    Trigger,
    Schedule,
    Improver,
    System,
}

/// Krawędź DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDep {
    pub task_id: String,
    /// `succeeded`, `failed`, `finished`, `output_equals`.
    pub condition: String,
}

/// Zadanie w panelu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
    pub parent_id: Option<String>,
    pub deps: Vec<TaskDep>,
    /// `alfa`, `role:coder`, `any`, `system:<usługa>`.
    pub assignee: String,
    pub agent: Option<String>,
    pub class: TaskClassKind,
    pub origin: TaskOriginKind,
    pub origin_detail: Option<String>,
    /// `agent`, `bridge:claude_code`, `bridge:codex`, `service:<usługa>`.
    pub executor: String,
    pub state: TaskStateKind,
    pub result: Option<TaskResultKind>,
    /// Powód czekania (PL), błąd albo przyczyna zakończenia.
    pub blocked: Option<String>,
    pub error: Option<String>,
    pub summary: Option<String>,
    pub attempt: u32,
    pub max_attempts: u32,
    pub steps: u32,
    pub max_steps: u32,
    pub cost: Money,
    pub session_id: Option<String>,
    pub tainted: bool,
    pub submitted_at: Iso8601,
    pub deadline_at: Iso8601,
}

/// Nowe zadanie od użytkownika (węzeł DAG: `after` = poprzedniczki, `parent_id` = delegacja).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTaskInput {
    pub session_id: Option<String>,
    pub title: String,
    pub goal: String,
    pub agent: Option<String>,
    pub after: Vec<String>,
    pub parent_id: Option<String>,
}

/// Rodzaj wyzwalacza.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TriggerKindView {
    Cron {
        expr: String,
    },
    Once {
        at: Iso8601,
    },
    Interval {
        every_minutes: u64,
    },
    FileInDir {
        dir: String,
        pattern: Option<String>,
    },
    NewMessage {
        session_id: Option<String>,
    },
    TaskFinished {
        task_prefix: Option<String>,
        /// `succeeded`, `failed`, `any`.
        outcome: String,
    },
    Manual,
}

/// Wyzwalacz w liście.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerInfo {
    pub id: String,
    pub name: String,
    pub kind: TriggerKindView,
    pub enabled: bool,
    /// `user`, `agent:<persona>`, `system:<usługa>`.
    pub owner: String,
    pub title: String,
    pub goal: String,
    pub agent: Option<String>,
    /// Most CLI (`claude_code` / `codex`) — tylko harmonogram użytkownika z jawną zgodą.
    pub bridge: Option<String>,
    pub tz: String,
    pub next_fire_at: Option<Iso8601>,
    pub last_fire_at: Option<Iso8601>,
    pub fired: u64,
    pub suppressed: u64,
    pub deferred_until: Option<Iso8601>,
    pub respect_dnd: bool,
    /// Obserwacja katalogów niedostępna na tej platformie (wyzwalacz plikowy tylko ręcznie).
    pub watch_unavailable: bool,
}

/// Nowy wyzwalacz (właściciel = użytkownik).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerDraft {
    pub name: String,
    pub kind: TriggerKindView,
    pub title: String,
    pub goal: String,
    pub agent: Option<String>,
    pub bridge: Option<String>,
    pub respect_dnd: bool,
}

/// Wpis dziennika uruchomień.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerRunInfo {
    pub at: Iso8601,
    pub trigger_id: String,
    /// Przyczyna (PL): „harmonogram", „ręcznie", „nowy plik: …", …
    pub cause: String,
    /// `submitted`, `suppressed`, `deferred`, `failed`.
    pub outcome: String,
    pub task_id: Option<String>,
    pub detail: Option<String>,
}

/// Podgląd wyrażenia cron (następne uruchomienia w strefie Europe/Warsaw).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronPreview {
    pub valid: bool,
    pub error: Option<String>,
    pub next: Vec<Iso8601>,
}

/// Reguła Marszałka (opis zawężeń po polsku + reguła źródłowa).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarshalRuleInfo {
    pub id: String,
    pub description: String,
    pub when: Vec<String>,
    pub effects: Vec<String>,
    pub rule: serde_json::Value,
}

/// Odrzucony szkic (nigdy nie wejdzie w życie).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarshalRejected {
    pub draft: serde_json::Value,
    pub errors: Vec<String>,
}

/// Propozycja reguł.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarshalProposalInfo {
    pub id: u64,
    pub text: String,
    pub rules: Vec<MarshalRuleInfo>,
    pub rejected: Vec<MarshalRejected>,
    pub conflicts: Vec<String>,
    /// `pending`, `approved`, `rejected`.
    pub status: String,
    pub created_at: Iso8601,
}

/// Stan Marszałka (Ustawienia → Agentki → Reguły).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarshalState {
    pub rules: Vec<MarshalRuleInfo>,
    pub proposals: Vec<MarshalProposalInfo>,
    /// Polityka efektywna (sufit ∩ reguły) w punktach.
    pub effective: Vec<String>,
    /// Tłumacz poleceń (model) dostępny — bez niego tylko reguły z edytora.
    pub translator: bool,
}

/// Raport dzienny Marszałka (relacjonuje Dyrygentka).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarshalReport {
    pub day: String,
    pub submitted: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub escalations: u64,
    pub text: String,
}
