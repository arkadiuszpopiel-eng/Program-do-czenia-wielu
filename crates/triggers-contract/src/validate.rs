//! Błędy i walidacja wyzwalaczy. Twarda reguła zgodności (PLAN §1.3 pkt 4, subscription-routes
//! §2.4, AGENTS.md „Czego nie wolno”): wyzwalacz — także harmonogram czasowy użytkownika — nigdy
//! nie celuje w most CLI; `allow_bridges` jest odrzucane (pole zostaje tylko dla zgodności odczytu
//! zapisanych wyzwalaczy; CX-d).

use scheduler_contract::{Assignee, ExecutorKind, Resource, TaskClass};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::{Actor, TriggerId, TriggerKind, TriggerSpec};

/// Najkrótszy interwał (1 min).
pub const MIN_INTERVAL_MS: u64 = 60_000;
/// Najdłuższy interwał (30 dni).
pub const MAX_INTERVAL_MS: u64 = 30 * 24 * 3_600_000;
/// Najwięcej wyzwalaczy.
pub const MAX_TRIGGERS: usize = 512;
/// Globalny limit wyzwoleń wszystkich wyzwalaczy na godzinę.
pub const GLOBAL_MAX_FIRES_PER_HOUR: usize = 120;
/// Najdłuższy łańcuch wyzwalaczy (zadanie z wyzwalacza → koniec → kolejny wyzwalacz…).
pub const MAX_CHAIN_DEPTH: u32 = 3;
/// Domyślny termin zadania z wyzwalacza (1 h).
pub const DEFAULT_TASK_DEADLINE_MS: u64 = 3_600_000;
/// Wpisów dziennika na wyzwalacz.
pub const LOG_PER_TRIGGER: usize = 100;
/// Wpisów dziennika łącznie.
pub const LOG_TOTAL: usize = 1_000;
/// Doba (ms).
pub const DAY_MS: u64 = 86_400_000;

/// Błąd operacji na wyzwalaczach.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum TriggerError {
    /// Niepoprawna specyfikacja.
    #[error("niepoprawny wyzwalacz {id}: {reason}")]
    Invalid {
        /// Wyzwalacz.
        id: TriggerId,
        /// Powód.
        reason: String,
    },
    /// Most CLI z wyzwalacza — zabronione.
    #[error("wyzwalacz {0} nie może uruchamiać mostu CLI")]
    BridgeForbidden(TriggerId),
    /// Brak uprawnień do operacji (właściciel).
    #[error("brak uprawnień: {0}")]
    Forbidden(String),
    /// Identyfikator zajęty.
    #[error("wyzwalacz {0} już istnieje")]
    Duplicate(TriggerId),
    /// Nieznany wyzwalacz.
    #[error("nieznany wyzwalacz {0}")]
    Unknown(TriggerId),
    /// Za dużo wyzwalaczy.
    #[error("przekroczono limit {0} wyzwalaczy")]
    Capacity(usize),
    /// Wyzwalacz wyłączony.
    #[error("wyzwalacz {0} jest wyłączony")]
    Disabled(TriggerId),
    /// Moduł nie działa.
    #[error("wyzwalacze nie są uruchomione")]
    NotStarted,
}

fn invalid(spec: &TriggerSpec, reason: impl Into<String>) -> TriggerError {
    TriggerError::Invalid {
        id: spec.id.clone(),
        reason: reason.into(),
    }
}

/// Czy `actor` może zarządzać wyzwalaczem właściciela `owner` (użytkownik — każdym).
pub fn may_manage(actor: &Actor, owner: &Actor) -> bool {
    actor == &Actor::User || actor == owner
}

/// Walidacja specyfikacji tworzonej/zmienianej przez `actor` w chwili `now_ms`.
pub fn validate(spec: &TriggerSpec, actor: &Actor, now_ms: u64) -> Result<(), TriggerError> {
    if !spec.id.is_valid() {
        return Err(invalid(spec, "niepoprawny identyfikator"));
    }
    if spec.name.trim().is_empty() || spec.name.chars().count() > 100 {
        return Err(invalid(spec, "nazwa pusta albo za długa"));
    }
    if actor != &spec.owner {
        return Err(TriggerError::Forbidden(
            "właścicielem wyzwalacza jest ten, kto go tworzy".into(),
        ));
    }
    validate_kind(spec, now_ms)?;
    validate_action(spec)?;
    validate_bridges(spec)?;
    let r = &spec.rate;
    if r.max_fires == 0 || r.max_fires > 1_000 || r.per_ms < 60_000 || r.per_ms > 7 * DAY_MS {
        return Err(invalid(spec, "limit częstości poza zakresem"));
    }
    if let Some(q) = &spec.quiet
        && (q.start_min >= 1440 || q.end_min >= 1440 || q.days.iter().any(|d| *d > 6))
    {
        return Err(invalid(spec, "niepoprawne okno ciszy"));
    }
    if spec.scope.len() > 32 {
        return Err(invalid(spec, "za szeroki zakres uprawnień"));
    }
    Ok(())
}

fn validate_kind(spec: &TriggerSpec, now_ms: u64) -> Result<(), TriggerError> {
    match &spec.kind {
        TriggerKind::Cron { expr } => {
            if expr.next_after(now_ms, &spec.tz).is_none() {
                return Err(invalid(spec, "wyrażenie cron nigdy nie wystąpi"));
            }
        }
        TriggerKind::Once { at_ms } => {
            if *at_ms <= now_ms {
                return Err(invalid(spec, "chwila jednorazowa już minęła"));
            }
        }
        TriggerKind::Interval { every_ms, .. } => {
            if !(MIN_INTERVAL_MS..=MAX_INTERVAL_MS).contains(every_ms) {
                return Err(invalid(spec, "interwał poza zakresem 1 min – 30 dni"));
            }
        }
        TriggerKind::FileInDir { dir, pattern } => {
            if dir.trim().is_empty() || pattern.as_ref().is_some_and(|p| p.len() > 128) {
                return Err(invalid(spec, "niepoprawny katalog albo wzorzec"));
            }
        }
        TriggerKind::TaskFinished { task_prefix, .. } => {
            if task_prefix.as_ref().is_some_and(|p| p.len() > 96) {
                return Err(invalid(spec, "za długi prefiks zadania"));
            }
        }
        TriggerKind::NewMessage { .. } | TriggerKind::Manual => {}
    }
    Ok(())
}

fn validate_action(spec: &TriggerSpec) -> Result<(), TriggerError> {
    let a = &spec.action;
    if a.title.trim().is_empty() || a.title.chars().count() > 200 {
        return Err(invalid(spec, "tytuł akcji pusty albo za długi"));
    }
    if a.goal.trim().is_empty() || a.goal.chars().count() > 4_000 {
        return Err(invalid(spec, "cel akcji pusty albo za długi"));
    }
    if a.class == TaskClass::User {
        return Err(invalid(
            spec,
            "wyzwalacz nie jest żądaniem użytkownika (klasa najwyżej `agent`)",
        ));
    }
    if matches!(a.assignee, Assignee::System(_)) && a.resources.contains(&Resource::Speaker) {
        return Err(invalid(spec, "usługa systemowa nie mówi"));
    }
    Ok(())
}

/// Most CLI z wyzwalacza — zawsze odmowa, bez wyjątku dla harmonogramu (CX-d).
fn validate_bridges(spec: &TriggerSpec) -> Result<(), TriggerError> {
    if spec.allow_bridges || matches!(spec.action.executor, ExecutorKind::Bridge(_)) {
        return Err(TriggerError::BridgeForbidden(spec.id.clone()));
    }
    Ok(())
}
