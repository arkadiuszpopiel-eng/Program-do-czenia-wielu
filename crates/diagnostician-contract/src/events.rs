//! Zdarzenia Diagnosty (strumień Diagnostyka; naprawy — także Audyt przez kompozycję).

use core_bus_contract::{Event, EventKind, Level};
use serde_json::Value;

/// Nowy incydent z propozycją naprawy.
pub const EVENT_INCIDENT: &str = "diagnostician.incident.detected";
/// Naprawa wykonana i zweryfikowana.
pub const EVENT_REPAIRED: &str = "diagnostician.repair.verified";
/// Naprawa nieudana (cofnięta).
pub const EVENT_FAILED: &str = "diagnostician.repair.failed";
/// Naprawa cofnięta na żądanie.
pub const EVENT_UNDONE: &str = "diagnostician.repair.undone";
/// Wymaga człowieka.
pub const EVENT_NEEDS_HUMAN: &str = "diagnostician.needs_human";

/// Zdarzenie Diagnosty.
pub fn diag_event(name: &str, level: Level, payload: Value) -> Event {
    Event::new(EventKind::Custom(name.to_owned()), level, payload)
}
