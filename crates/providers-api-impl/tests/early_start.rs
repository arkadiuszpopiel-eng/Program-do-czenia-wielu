//! Przegląd 2026-10-22: Chat Completions zgłasza `Started` zaraz po przyjęciu żądania — model
//! z myśleniem (Gemini 2.5, Grok, o-series) wysyła pierwszą porcję po kilku sekundach, a Router
//! mierzy termin do pierwszego zdarzenia i przełączał go na model lokalny. Inny model w porcji
//! (OpenRouter wybiera konkretny) przychodzi jako zmiana modelu — koszt idzie za nim.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use providers_api_impl::{AuthScheme, OpenAiOptions, OpenAiProvider};
use providers_contract::{
    CancellationToken, ChatRequest, Message, ModelProvider, ProviderEvent, StaticKey,
    TurnAccumulator,
};
use support::openai_sse as sse;
use support::{FixtureServer, Part, Reply, http, profile};

fn provider(server: &FixtureServer, model: &str) -> OpenAiProvider {
    // Limity czasu adaptera dłuższe niż przerwa „myślenia” w odpowiedzi atrapy.
    let mut config = http(server, AuthScheme::Bearer);
    config.timeouts.first_token = Duration::from_secs(5);
    config.timeouts.idle = Duration::from_secs(5);
    OpenAiProvider::new(
        profile("compat", model),
        config,
        Arc::new(StaticKey::new("k")),
        OpenAiOptions::compatible(),
    )
    .unwrap()
}

#[tokio::test]
async fn started_comes_on_accept_not_after_thinking() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse_parts(vec![
        Part::Sleep(Duration::from_millis(1_500)),
        Part::Bytes(sse::chat_text("gemini-x", &["ok"], "stop").into_bytes()),
    ]));
    let p = provider(&server, "gemini-x");
    let t0 = Instant::now();
    let mut stream = p.stream(
        ChatRequest::new("gemini-x", vec![Message::user_text("x")]),
        CancellationToken::new(),
    );
    let first = stream.next().await.unwrap();
    assert!(
        t0.elapsed() < Duration::from_millis(1_000),
        "{:?}",
        t0.elapsed()
    );
    assert!(matches!(&first, ProviderEvent::Started { model, .. } if model == "gemini-x"));
    let rest: Vec<ProviderEvent> = stream.collect().await;
    assert!(
        !rest.iter().any(|e| matches!(
            e,
            ProviderEvent::Started { .. } | ProviderEvent::ModelSwitched { .. }
        )),
        "jedno Started: {rest:?}"
    );
    assert!(
        t0.elapsed() >= Duration::from_millis(1_400),
        "porcja po przerwie: {:?}",
        t0.elapsed()
    );
    assert_eq!(
        rest.last(),
        Some(&ProviderEvent::stop(
            providers_contract::StopReason::EndTurn
        )),
        "{rest:?}"
    );
}

#[tokio::test]
async fn different_model_in_stream_becomes_a_switch() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(sse::chat_text(
        "anthropic/claude-x",
        &["ok"],
        "stop",
    )));
    let p = provider(&server, "openrouter/auto");
    let events: Vec<ProviderEvent> = p
        .stream(
            ChatRequest::new("openrouter/auto", vec![Message::user_text("x")]),
            CancellationToken::new(),
        )
        .collect()
        .await;
    assert!(
        matches!(&events[0], ProviderEvent::Started { model, .. } if model == "openrouter/auto")
    );
    let mut acc = TurnAccumulator::new(p.id().clone());
    events.iter().for_each(|e| acc.push(e));
    let turn = acc.finish();
    assert_eq!(turn.model.as_deref(), Some("anthropic/claude-x"));
    assert_eq!(turn.message.visible_text(), "ok");
}
