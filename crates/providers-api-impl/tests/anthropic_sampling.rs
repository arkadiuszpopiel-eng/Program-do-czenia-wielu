//! Adapter Anthropic: temperatura a myślenie. API nie przyjmuje zmiany temperatury razem
//! z myśleniem (dokumentacja „extended thinking”), a Claude 4.6 ma myślenie opcjonalne, domyślnie
//! włączone — temperatura z ustawień (P4, od serii 8 także w przebiegach agentek) nie może wtedy
//! iść w żądaniu (przegląd 2026-10-21).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use futures_util::StreamExt;
use providers_api_impl::{AnthropicOptions, AnthropicProvider, AuthScheme};
use providers_contract::{
    CancellationToken, ChatRequest, Message, ModelProvider, ProviderEvent, StaticKey,
};
use serde_json::{Value, json};
use support::anthropic_sse as sse;
use support::{FixtureServer, Reply, http, profile};

const MODEL: &str = "claude-sonnet-4-6";

async fn body_of(server: &FixtureServer, req: ChatRequest) -> Value {
    server.reset(Reply::sse(sse::text(MODEL, &["ok"])));
    let p = AnthropicProvider::new(
        profile("anthropic", MODEL),
        http(server, AuthScheme::XApiKey),
        Arc::new(StaticKey::new("sk-ant-test-123")),
        AnthropicOptions::native(),
    )
    .unwrap();
    let _: Vec<ProviderEvent> = p.stream(req, CancellationToken::new()).collect().await;
    server.requests().pop().unwrap().json()
}

#[tokio::test]
async fn temperature_only_without_thinking() {
    let server = FixtureServer::start().await;
    let mut req = ChatRequest::new(MODEL, vec![Message::user_text("hej")]);
    req.params.temperature = Some(0.2);
    // Domyślnie myślenie włączone: bez temperatury.
    let thinking = body_of(&server, req.clone()).await;
    assert_eq!(thinking["thinking"], json!({"type": "adaptive"}));
    assert!(thinking.get("temperature").is_none(), "{thinking}");
    // Myślenie wyłączone: temperatura z ustawień idzie.
    req.params.thinking.enabled = false;
    let plain = body_of(&server, req).await;
    assert_eq!(plain["thinking"], json!({"type": "disabled"}));
    assert_eq!(plain["temperature"], json!(0.2_f32));
}
