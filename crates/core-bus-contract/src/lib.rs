//! Kontrakt magistrali zdarzeń jądra Alfy (docs/PLAN.md §13, docs/modules/core-bus/SPEC.md).
//!
//! Crate zawiera wyłącznie typy, trait `EventBus` i JSON Schema — bez implementacji.
//! Implementacje: `core-bus-impl` (produkcyjna, `tokio::sync::broadcast`) i `core-bus-fake`
//! (deterministyczna, do testów). Inne moduły zależą **tylko** od tego crate'a.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod bus;
mod event;
mod filter;
mod schema;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use bus::{BusError, BusItem, BusStats, EventBus, EventStream};
pub use event::{AgentId, Cost, Event, EventKind, Level, RunId, SessionId, SpanId};
pub use filter::EventFilter;
pub use schema::{event_schema, event_schema_json, EVENT_SCHEMA_VERSION};
