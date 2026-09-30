//! Round-trip serde pełnego zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_bus_contract::{AgentId, Cost, Event, EventKind, Level, RunId, SessionId, SpanId};

#[test]
fn full_event_round_trips_through_json() {
    let ev = Event::new(
        EventKind::ModelCall,
        Level::Info,
        serde_json::json!({"provider": "anthropic", "model": "x"}),
    )
    .with_session(SessionId::from("s"))
    .with_agent(AgentId::from("a"))
    .with_run(RunId::from("r"))
    .with_span(SpanId::from("sp"))
    .with_cost(Cost {
        input_tokens: 1,
        output_tokens: 2,
        micro_usd: 3,
        latency_ms: Some(4),
    });
    let json = serde_json::to_string(&ev).unwrap();
    let back: Event = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ev);
}

#[test]
fn minimal_event_omits_optional_fields() {
    let ev = Event::new(
        EventKind::Custom("m.x".into()),
        Level::Trace,
        serde_json::Value::Null,
    );
    let value = serde_json::to_value(&ev).unwrap();
    let obj = value.as_object().unwrap();
    for absent in ["session", "agent", "run", "span", "cost", "prev_hash"] {
        assert!(
            !obj.contains_key(absent),
            "pole {absent} nie powinno być serializowane"
        );
    }
    assert_eq!(value["kind"], serde_json::json!("m.x"));
    assert_eq!(value["level"], serde_json::json!("trace"));
}

#[test]
fn unknown_level_is_rejected() {
    let json = r#"{"id":"6f9619ff-8b86-d011-b42d-00c04fc964ff","ts":"2026-01-01T00:00:00Z","kind":"ui","level":"loud","payload":null}"#;
    assert!(serde_json::from_str::<Event>(json).is_err());
}
