//! JSON Schema neutralnego IR (do generowania typów TS i walidacji kaset NDJSON atrapy).

use crate::event::ProviderEvent;
use crate::request::ChatRequest;

/// Wersja schematu IR (podbijana przy zmianie niezgodnej wstecz; upcastery wg PLAN §13).
pub const IR_SCHEMA_VERSION: u32 = 1;

/// Schemat `ChatRequest`.
pub fn chat_request_schema() -> schemars::Schema {
    schemars::schema_for!(ChatRequest)
}

/// Schemat `ProviderEvent`.
pub fn provider_event_schema() -> schemars::Schema {
    schemars::schema_for!(ProviderEvent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_have_titles_and_definitions() {
        let req = serde_json::to_value(chat_request_schema()).unwrap_or_default();
        assert_eq!(req["title"], "ChatRequest");
        assert!(req["$defs"]["Message"].is_object());
        let ev = serde_json::to_value(provider_event_schema()).unwrap_or_default();
        assert_eq!(ev["title"], "ProviderEvent");
        assert!(ev.to_string().contains("thinking_signature"));
    }
}
