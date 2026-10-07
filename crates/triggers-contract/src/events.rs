//! Zdarzenia magistrali `triggers.*` (Oś czasu, Audyt zmian, Marszałek). Ładunki: identyfikatory,
//! rodzaje, powody — bez celu akcji i bez treści niezaufanej (ścieżka pliku tylko jako nazwa).

use core_bus_contract::{Event, EventKind, Level};
use serde_json::{Value, json};

use crate::record::{FireCause, RunOutcome, RunRecord};
use crate::spec::{Actor, TriggerId};

/// Utworzono wyzwalacz.
pub const EVENT_CREATED: &str = "triggers.trigger.created";
/// Zmieniono wyzwalacz.
pub const EVENT_UPDATED: &str = "triggers.trigger.updated";
/// Usunięto wyzwalacz.
pub const EVENT_REMOVED: &str = "triggers.trigger.removed";
/// Włączono/wyłączono.
pub const EVENT_TOGGLED: &str = "triggers.trigger.toggled";
/// Wyzwolono — zadanie zgłoszone.
pub const EVENT_FIRED: &str = "triggers.fired";
/// Pominięto (limit, cisza, łańcuch, zaległość).
pub const EVENT_SUPPRESSED: &str = "triggers.suppressed";
/// Odłożono do końca ciszy.
pub const EVENT_DEFERRED: &str = "triggers.deferred";
/// Scheduler odrzucił zadanie.
pub const EVENT_FAILED: &str = "triggers.failed";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Zdarzenie zmiany wyzwalacza.
pub fn change_event(name: &str, id: &TriggerId, actor: &Actor, at_ms: u64, extra: Value) -> Event {
    let mut payload = json!({ "trigger": id, "actor": actor, "at_ms": at_ms });
    if let (Value::Object(p), Value::Object(e)) = (&mut payload, extra) {
        p.extend(e);
    }
    Event::new(event_kind(name), Level::Info, payload)
}

fn cause_json(cause: &FireCause) -> Value {
    match cause {
        FireCause::File { path } => {
            let name = path.rsplit(['/', '\\']).next().unwrap_or_default();
            json!({ "cause": "file", "name": name.chars().take(80).collect::<String>() })
        }
        FireCause::Deferred { original } => {
            let mut v = cause_json(original);
            v["deferred"] = json!(true);
            v
        }
        other => serde_json::to_value(other).unwrap_or(Value::Null),
    }
}

/// Zdarzenie wpisu dziennika uruchomień.
pub fn run_event(record: &RunRecord) -> Event {
    let (name, level) = match record.outcome {
        RunOutcome::Submitted { .. } => (EVENT_FIRED, Level::Info),
        RunOutcome::Suppressed { .. } => (EVENT_SUPPRESSED, Level::Info),
        RunOutcome::Deferred { .. } => (EVENT_DEFERRED, Level::Info),
        RunOutcome::Failed { .. } => (EVENT_FAILED, Level::Warn),
    };
    let payload = json!({
        "trigger": record.trigger,
        "at_ms": record.at_ms,
        "cause": cause_json(&record.cause),
        "outcome": record.outcome,
    });
    Event::new(event_kind(name), level, payload)
}
