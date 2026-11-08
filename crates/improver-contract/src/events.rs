//! Zdarzenia Ulepszacza (samo-zmiany — do Audytu przez kompozycję).

use core_bus_contract::{Event, EventKind, Level};
use serde_json::Value;

/// Propozycja utworzona.
pub const EVENT_PROPOSED: &str = "improver.proposal.created";
/// Próba zablokowana przez strażnika.
pub const EVENT_BLOCKED: &str = "improver.proposal.blocked";
/// Wynik oceny (piaskownica + holdout, zbiorczo).
pub const EVENT_EVALUATED: &str = "improver.proposal.evaluated";
/// Wdrożono.
pub const EVENT_DEPLOYED: &str = "improver.change.deployed";
/// Cofnięto.
pub const EVENT_ROLLED_BACK: &str = "improver.change.rolled_back";
/// Odrzucono.
pub const EVENT_REJECTED: &str = "improver.proposal.rejected";

/// Zdarzenie Ulepszacza.
pub fn improver_event(name: &str, level: Level, payload: Value) -> Event {
    Event::new(EventKind::Custom(name.to_owned()), level, payload)
}
