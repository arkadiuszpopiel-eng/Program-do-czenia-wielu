//! `router.fallback` → konto zapasowe: tylko przełączenie między kontami, bez błędów przejściowych.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_files::projects::handover;

#[test]
fn handover_only_between_accounts_and_not_for_transient_errors() {
    let brain = |m: &str| {
        let account = m
            .split_once("--")
            .map(|(_, a)| a.split(':').next().unwrap_or(""));
        Some(app_api::ports::BrainTarget {
            provider_id: "openai".into(),
            provider_name: "OpenAI".into(),
            account: Some(account.unwrap_or("a1").to_owned()),
            model: "m".into(),
            local: false,
            context_window: None,
        })
    };
    let event = |kind: serde_json::Value, to: &str| serde_json::json!({ "from": "openai:m", "to": to, "cause": { "cause": "error", "kind": kind } });
    let limited = event(
        serde_json::json!({ "kind": "rate_limited", "retry_after_ms": 1000 }),
        "openai--a2:m",
    );
    let (account, error) = handover(&limited, brain).expect("limit");
    assert_eq!(
        (account.as_str(), error.code),
        ("a1", app_api::dto::TurnErrorCode::RateLimited)
    );
    assert!(error.retry_at.is_some());
    let auth = event(serde_json::json!({ "kind": "auth" }), "openai--a2:m");
    assert_eq!(
        handover(&auth, brain).map(|h| h.1.code),
        Some(app_api::dto::TurnErrorCode::Provider)
    );
    for transient in [
        serde_json::json!({ "kind": "overloaded" }),
        serde_json::json!({ "kind": "server", "status": 503 }),
    ] {
        assert!(handover(&event(transient, "openai--a2:m"), brain).is_none());
    }
    // To samo konto (inny model) — nie ma przełączenia.
    assert!(
        handover(
            &event(serde_json::json!({ "kind": "auth" }), "openai:m"),
            brain
        )
        .is_none()
    );
}
