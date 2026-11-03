//! Kształt DTO w IPC zgodny z TypeScriptem: ramki terminala, profile, czas ISO, kwoty i poziomy.

#![allow(clippy::unwrap_used)]

use app_api::dto::{AutonomyLevel, ModelProfile, Money, TerminalFrame, TerminalProfileId, iso};

#[test]
fn frames_and_profiles_have_ipc_shape() {
    let f = TerminalFrame::Output {
        data_b64: "YQ==".into(),
    };
    assert_eq!(
        serde_json::to_value(&f).unwrap(),
        serde_json::json!({"kind": "output", "data_b64": "YQ=="})
    );
    assert_eq!(
        serde_json::to_value(TerminalFrame::Exit { code: None }).unwrap(),
        serde_json::json!({"kind": "exit", "code": null})
    );
    assert_eq!(
        serde_json::to_value(TerminalProfileId::ClaudeLogin).unwrap(),
        serde_json::json!("claude_login")
    );
}

#[test]
fn iso_matches_js_to_iso_string() {
    let ts = chrono::DateTime::parse_from_rfc3339("2026-09-30T08:45:00Z")
        .map(|t| t.with_timezone(&chrono::Utc));
    assert_eq!(
        ts.map(iso).ok().as_deref(),
        Some("2026-09-30T08:45:00.000Z")
    );
}

#[test]
fn money_and_levels_serialize_like_ts() {
    let json = serde_json::to_value(Money::pln(214)).ok();
    assert_eq!(
        json,
        Some(serde_json::json!({"minor": 214, "currency": "PLN"}))
    );
    assert_eq!(
        serde_json::to_value(AutonomyLevel::L3).ok(),
        Some(serde_json::json!("L3"))
    );
    assert_eq!(ModelProfile::parse("cloud"), Some(ModelProfile::Cloud));
}
