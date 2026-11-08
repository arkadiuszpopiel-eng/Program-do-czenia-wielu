//! Przegląd 2026-10-22: oficjalne OpenAI zna sufit limitu wyjścia rodzin modeli — zadanie agentki
//! (jawne `max_tokens`) dostaje 16 384 tokeny zamiast 8 192, a rozmowa bez żądanego limitu zostaje
//! przy domyślnych 8 192 (sufit jako wartość domyślna dawał 400 dla `gpt-5-chat-latest`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use futures_util::StreamExt;
use providers_api_impl::{AuthScheme, OpenAiOptions, OpenAiProvider, ProviderProfile};
use providers_contract::{CancellationToken, ChatRequest, Message, ModelProvider, StaticKey};
use support::openai_sse as sse;
use support::{FixtureServer, Reply, http};

async fn sent(server: &FixtureServer, model: &str, requested: Option<u32>) -> u64 {
    let p = OpenAiProvider::new(
        ProviderProfile::new("openai"),
        http(server, AuthScheme::Bearer),
        Arc::new(StaticKey::new("sk-oai")),
        OpenAiOptions::native(),
    )
    .unwrap();
    server.reset(Reply::sse(
        sse::resp_created(model) + &sse::resp_completed(model),
    ));
    let mut req = ChatRequest::new(model, vec![Message::user_text("x")]);
    req.params.max_tokens = requested;
    let _: Vec<_> = p.stream(req, CancellationToken::new()).collect().await;
    server.requests().pop().unwrap().json()["max_output_tokens"]
        .as_u64()
        .unwrap()
}

#[tokio::test]
async fn family_ceiling_applies_only_to_requested_limits() {
    let server = FixtureServer::start().await;
    for m in ["gpt-5", "gpt-5-chat-latest", "o3", "gpt-4.1", "gpt-4o"] {
        assert_eq!(sent(&server, m, Some(16_384)).await, 16_384, "{m}");
        // Rozmowa bez żądanego limitu — jak wcześniej, domyślny profilu.
        assert_eq!(sent(&server, m, None).await, 8_192, "{m}");
    }
    // Sufit rodziny ogranicza większą prośbę.
    assert_eq!(sent(&server, "gpt-4o", Some(32_768)).await, 16_384);
    assert_eq!(sent(&server, "o1-preview", Some(65_536)).await, 32_768);
    // Nieznana rodzina i stary snapshot — domyślny profilu, jak wcześniej.
    assert_eq!(sent(&server, "gpt-x", Some(16_384)).await, 8_192);
    assert_eq!(
        sent(&server, "gpt-4o-2024-05-13", Some(16_384)).await,
        8_192
    );
}
