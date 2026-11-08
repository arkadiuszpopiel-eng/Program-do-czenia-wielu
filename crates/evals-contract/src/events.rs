//! Zdarzenia magistrali modułu `evals` (strumień Diagnostyka; decyzje bramki także Audyt).

use core_bus_contract::{Event, EventKind, Level};

/// Przebieg zestawu zakończony; ładunek: raport zbiorczy (bez przypadków holdoutu).
pub const EVENT_RUN_COMPLETED: &str = "evals.run.completed";
/// Decyzja bramki ewaluacyjnej; ładunek: [`crate::GateVerdict`] (wynik zbiorczy).
pub const EVENT_GATE_DECIDED: &str = "evals.gate.decided";
/// Naruszona integralność zestawu zamrożonego albo holdoutu; ładunek: `{suite, mismatched, missing}`.
pub const EVENT_INTEGRITY_FAILED: &str = "evals.integrity.failed";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Zdarzenie modułu `evals`.
pub fn evals_event(name: &str, level: Level, payload: serde_json::Value) -> Event {
    Event::new(event_kind(name), level, payload)
}
