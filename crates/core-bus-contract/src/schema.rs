//! JSON Schema zdarzenia — źródło dla `packages/schemas/event.v1.json`.

use crate::event::Event;

/// Wersja schematu zdarzenia; zmiana niezgodna wstecz = nowa wersja pliku + upcaster (PLAN §13).
pub const EVENT_SCHEMA_VERSION: u32 = 1;

/// Schemat zdarzenia (schemars).
pub fn event_schema() -> schemars::Schema {
    let mut schema = schemars::schema_for!(Event);
    if let Some(obj) = schema.as_object_mut() {
        obj.insert(
            "$id".into(),
            serde_json::Value::String(format!("alfa://schemas/event.v{EVENT_SCHEMA_VERSION}.json")),
        );
        obj.insert(
            "x-schema-version".into(),
            serde_json::Value::from(EVENT_SCHEMA_VERSION),
        );
    }
    schema
}

/// Schemat jako sformatowany JSON (z końcowym znakiem nowej linii — jak w repo).
pub fn event_schema_json() -> String {
    let mut text = serde_json::to_string_pretty(&event_schema()).unwrap_or_default();
    text.push('\n');
    text
}
