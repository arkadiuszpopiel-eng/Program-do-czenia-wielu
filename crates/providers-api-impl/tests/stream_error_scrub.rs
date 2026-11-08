//! Regresja Q-7: błąd dostawcy w trakcie strumienia SSE przechodzi przez redakcję klucza
//! i limit długości komunikatu — tak samo jak błąd HTTP przed strumieniem.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use futures_util::StreamExt;
use providers_api_impl::{AuthScheme, OpenAiOptions, OpenAiProvider};
use providers_contract::{
    CancellationToken, ChatRequest, Message, ModelProvider, ProviderEvent, StaticKey,
};
use serde_json::json;
use support::openai_sse as sse;
use support::{FixtureServer, Reply, http, profile};

const MODEL: &str = "gpt-6-luna";
const KEY: &str = "sk-oai-test-klucz-tajny";

#[tokio::test]
async fn mid_stream_provider_error_is_scrubbed() {
    let server = FixtureServer::start().await;
    let provider = OpenAiProvider::new(
        profile("openai", MODEL),
        http(&server, AuthScheme::Bearer),
        Arc::new(StaticKey::new(KEY)),
        OpenAiOptions::compatible(),
    )
    .unwrap();
    let message = format!("zły klucz {KEY} {}", "x".repeat(2_000));
    server.reset(Reply::sse(
        sse::chat_role(MODEL)
            + &sse::chat_text_delta(MODEL, "Zaczynam")
            + &sse::data(&json!({"error": {"message": message, "type": "server_error"}})),
    ));
    let events: Vec<ProviderEvent> = provider
        .stream(
            ChatRequest::new(MODEL, vec![Message::user_text("x")]),
            CancellationToken::new(),
        )
        .collect()
        .await;
    let Some(ProviderEvent::Error(err)) = events.last() else {
        panic!("oczekiwano błędu na końcu strumienia: {events:?}");
    };
    assert!(!err.message.contains(KEY), "klucz w komunikacie błędu");
    assert!(err.message.chars().count() <= 501, "komunikat bez limitu");
    assert!(err.after_output, "błąd po treści");
}
