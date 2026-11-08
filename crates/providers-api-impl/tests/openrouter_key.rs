//! OpenRouter: lista modeli jest publiczna, więc klucz sprawdza osobne `GET /key`
//! (zgłoszenie właściciela 2026-10-14: „Testuj połączenie” przy OpenRouterze nic nie robił).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use providers_api_impl::{
    AuthScheme, OpenAiOptions, OpenAiProvider, ProviderProfile, openrouter_key_note,
};
use providers_contract::{ModelProvider, ProviderErrorKind, StaticKey};
use serde_json::json;
use support::{FixtureServer, Reply, http, profile};

fn provider(server: &FixtureServer, profile: ProviderProfile) -> OpenAiProvider {
    OpenAiProvider::new(
        profile,
        http(server, AuthScheme::Bearer),
        Arc::new(StaticKey::new("sk-or-test")),
        OpenAiOptions::compatible(),
    )
    .unwrap()
}

#[tokio::test]
async fn openrouter_checks_the_key_with_get_key() {
    let server = FixtureServer::start().await;
    server.reset(Reply::json(
        200,
        &json!({"data": {"label": "sk-or-v1-abc...xyz", "limit": 50.0, "usage": 1.234,
                         "is_free_tier": false, "rate_limit": {"requests": 10, "interval": "10s"}}}),
    ));
    let p = provider(&server, profile("openrouter", "openai/gpt-6-luna"));
    let note = p.check_key().await.unwrap();
    assert_eq!(
        note.as_deref(),
        Some("limit klucza 50,00 USD, zużyto 1,23 USD")
    );
    let sent = &server.requests()[0];
    assert_eq!(sent.path, "/key");
    assert_eq!(sent.method, "GET");
}

#[tokio::test]
async fn openrouter_rejected_key_is_an_auth_error() {
    let server = FixtureServer::start().await;
    server.reset(Reply::json(
        401,
        &json!({"error": {"message": "No auth credentials found", "code": 401}}),
    ));
    let p = provider(&server, profile("openrouter", "openai/gpt-6-luna"));
    let error = p.check_key().await.unwrap_err();
    assert!(matches!(error.kind, ProviderErrorKind::Auth), "{error:?}");
    assert_eq!(error.status, Some(401));
}

#[tokio::test]
async fn other_providers_have_no_separate_key_check() {
    let server = FixtureServer::start().await;
    server.reset(Reply::json(200, &json!({"data": {}})));
    let p = provider(&server, profile("mistral", "mistral-large"));
    let error = p.check_key().await.unwrap_err();
    assert!(matches!(error.kind, ProviderErrorKind::Unsupported));
    assert!(server.requests().is_empty(), "bez ruchu sieciowego");
}

#[test]
fn key_note_describes_limit_usage_and_free_tier() {
    assert_eq!(
        openrouter_key_note(&json!({"limit": null, "usage": 0, "is_free_tier": true})),
        "klucz bez limitu, zużyto 0,00 USD · konto darmowe (bez doładowania)"
    );
    assert_eq!(
        openrouter_key_note(&json!({})),
        "klucz bez limitu, zużyto 0,00 USD"
    );
}
