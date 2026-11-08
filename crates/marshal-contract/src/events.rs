//! Zdarzenia magistrali `marshal.*` (Oś czasu, powiadomienia, Dyrygentka relacjonuje).

use core_bus_contract::{Event, EventKind, Level};
use serde_json::{Value, json};

use crate::watch::{Escalation, Severity};

/// Propozycja reguł (z konfliktami i odrzuconymi szkicami).
pub const EVENT_PROPOSED: &str = "marshal.rule.proposed";
/// Reguły zatwierdzone przez użytkownika.
pub const EVENT_APPROVED: &str = "marshal.rule.approved";
/// Propozycja odrzucona.
pub const EVENT_REJECTED: &str = "marshal.rule.rejected";
/// Reguła cofnięta przez użytkownika.
pub const EVENT_REVOKED: &str = "marshal.rule.revoked";
/// Eskalacja do użytkownika.
pub const EVENT_ESCALATION: &str = "marshal.escalation";
/// Raport dzienny.
pub const EVENT_REPORT: &str = "marshal.report.daily";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Zdarzenie Marszałka.
pub fn marshal_event(name: &str, at_ms: u64, mut payload: Value) -> Event {
    if let Value::Object(map) = &mut payload {
        map.insert("at_ms".into(), json!(at_ms));
    }
    Event::new(event_kind(name), Level::Info, payload)
}

/// Zdarzenie eskalacji.
pub fn escalation_event(e: &Escalation) -> Event {
    let level = match e.severity {
        Severity::Info => Level::Info,
        Severity::Warn => Level::Warn,
    };
    Event::new(
        event_kind(EVENT_ESCALATION),
        level,
        serde_json::to_value(e).unwrap_or(Value::Null),
    )
}
