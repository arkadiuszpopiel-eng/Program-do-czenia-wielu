//! Adapter Anthropic: ponawianie przed pierwszym tokenem, odzyskanie po odrzuceniu podpisu
//! myślenia, błędy w trakcie strumienia, redakcja klucza, brak klucza, Models API, anulowanie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use providers_api_impl::{AnthropicOptions, AnthropicProvider, AuthScheme};
use providers_contract::{
    CancellationToken, ChatRequest, ContentBlock, HealthState, Message, ModelProvider,
    ProviderErrorKind, ProviderEvent, ProviderOrigin, Role, StaticKey, StopReason, ThinkingBlock,
    ThinkingSupport,
};
use serde_json::json;
use support::anthropic_sse::{self as sse, Block};
use support::{FixtureServer, Part, Reply, http, http_error, profile, slow};
use tokio::time::Instant;

const MODEL: &str = "claude-opus-5-5";

fn provider(server: &FixtureServer, key: StaticKey) -> AnthropicProvider {
    AnthropicProvider::new(
        profile("anthropic", MODEL),
        http(server, AuthScheme::XApiKey),
        Arc::new(key),
        AnthropicOptions::native(),
    )
    .unwrap()
}

fn key() -> StaticKey {
    StaticKey::new("sk-ant-secret-777")
}

async fn run(p: &AnthropicProvider, req: ChatRequest) -> Vec<ProviderEvent> {
    p.stream(req, CancellationToken::new()).collect().await
}

fn req() -> ChatRequest {
    ChatRequest::new(MODEL, vec![Message::user_text("hej")])
}

#[tokio::test]
async fn overloaded_and_rate_limited_are_retried_before_first_token() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(sse::text(MODEL, &["po ponowieniu"])));
    server.push(http_error(
        529,
        None,
        &sse::error_body("overloaded_error", "Overloaded"),
    ));
    let p = provider(&server, key());
    let events = run(&p, req()).await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
    assert_eq!(server.requests().len(), 2);

    server.reset(Reply::sse(sse::text(MODEL, &["ok"])));
    server.push(
        http_error(429, None, &sse::error_body("rate_limit_error", "slow down"))
            .with_header("retry-after", "0.05"),
    );
    let start = Instant::now();
    let events = run(&p, req()).await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
    assert!(
        start.elapsed() >= Duration::from_millis(50),
        "retry-after respektowany"
    );
    assert_eq!(p.health().state, HealthState::Healthy);
}

#[tokio::test]
async fn persistent_5xx_fails_fast_for_router_fallback() {
    let server = FixtureServer::start().await;
    server.reset(http_error(
        500,
        None,
        &sse::error_body("api_error", "Internal"),
    ));
    let p = provider(&server, key());
    let start = Instant::now();
    let events = run(&p, req()).await;
    match events.as_slice() {
        [ProviderEvent::Error(e)] => {
            assert_eq!(e.kind, ProviderErrorKind::Server { status: 500 });
            assert!(e.should_fallback());
            assert_eq!(e.request_id.as_deref(), Some("req_011Fixture"));
            assert_eq!(e.provider_code.as_deref(), Some("api_error"));
        }
        other => panic!("{other:?}"),
    }
    assert!(start.elapsed() < Duration::from_secs(2), "ACC F1-04: ≤ 2 s");
    assert_eq!(
        server.requests().len(),
        2,
        "jedno ponowienie z polityki testowej"
    );
    assert_eq!(p.health().state, HealthState::Degraded);
}

#[tokio::test]
async fn thinking_binding_rejection_retries_once_without_thinking() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(sse::text(MODEL, &["ok"])));
    let msg = "messages.1.content.0: Invalid `signature` in `thinking` block. The block is bound to a different conversation. Remove the block, or set `thinking.block_binding.prefix_mismatch_behavior` to \"drop_block\".";
    server.push(http_error(
        400,
        None,
        &sse::error_body("invalid_request_error", msg),
    ));
    let p = provider(&server, key());
    let assistant = Message::new(
        Role::Assistant,
        vec![
            ContentBlock::Thinking(ThinkingBlock {
                text: String::new(),
                signature: Some("sig-stale".into()),
                provider_origin: ProviderOrigin {
                    provider: "anthropic".into(),
                    model: MODEL.into(),
                },
            }),
            ContentBlock::text("a"),
        ],
    );
    let r = ChatRequest::new(
        MODEL,
        vec![Message::user_text("q"), assistant, Message::user_text("q2")],
    );
    let events = run(&p, r).await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
    let reqs = server.requests();
    assert_eq!(reqs.len(), 2);
    assert!(reqs[0].body.contains("sig-stale"));
    assert!(
        !reqs[1].body.contains("sig-stale"),
        "druga próba bez bloków myślenia"
    );
    assert!(
        reqs[1].body.contains("\"text\":\"a\""),
        "tekst tury zostaje"
    );
}

#[tokio::test]
async fn mid_stream_error_is_marked_after_output() {
    let server = FixtureServer::start().await;
    let body = sse::message_start(MODEL)
        + &sse::block(0, &Block::Text(vec!["Część".into()]))
        + &sse::event("error", &sse::error_body("overloaded_error", "Overloaded"));
    server.reset(Reply::sse(body));
    let p = provider(&server, key());
    let events = run(&p, req()).await;
    match events.last() {
        Some(ProviderEvent::Error(e)) => {
            assert!(matches!(e.kind, ProviderErrorKind::Overloaded { .. }));
            assert!(e.after_output && !e.is_retryable());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        server.requests().len(),
        1,
        "po pierwszym tokenie brak ponowień"
    );

    server.reset(Reply::sse(
        sse::message_start(MODEL) + &sse::block(0, &Block::Text(vec!["urwane".into()])),
    ));
    let events = run(&p, req()).await;
    assert!(
        matches!(events.last(), Some(ProviderEvent::Error(e)) if e.kind == ProviderErrorKind::Protocol && e.after_output)
    );
}

#[tokio::test]
async fn key_never_leaks_and_missing_key_is_unconfigured() {
    let server = FixtureServer::start().await;
    server.reset(http_error(
        401,
        None,
        &sse::error_body(
            "authentication_error",
            "invalid x-api-key sk-ant-secret-777",
        ),
    ));
    let p = provider(&server, key());
    let events = run(&p, req()).await;
    let text = format!("{events:?}");
    assert!(!text.contains("secret-777"), "{text}");
    assert!(text.contains("[REDACTED]"));
    assert_eq!(p.health().state, HealthState::Unavailable);

    let empty = FixtureServer::start().await;
    let p = provider(&empty, StaticKey::none());
    assert_eq!(p.health().state, HealthState::Unconfigured);
    let events = run(&p, req()).await;
    assert!(
        matches!(events.as_slice(), [ProviderEvent::Error(e)] if e.kind == ProviderErrorKind::Auth)
    );
    assert!(empty.requests().is_empty());
}

#[tokio::test]
async fn models_api_paginates_and_teaches_capabilities() {
    let server = FixtureServer::start().await;
    let page1 = json!({"data": [{"id": "claude-new-9", "display_name": "Claude New 9", "created_at": "2026-09-20T00:00:00Z",
        "max_input_tokens": 500_000, "max_tokens": 64_000,
        "capabilities": {"image_input": {"supported": true}, "thinking": {"types": {"adaptive": {"supported": true}}},
                         "effort": {"supported": true}, "structured_outputs": {"supported": true}}}],
        "has_more": true, "first_id": "claude-new-9", "last_id": "claude-new-9"});
    let page2 = json!({"data": [{"id": MODEL, "display_name": "Claude Opus 5.5"}], "has_more": false, "last_id": MODEL});
    server.reset(Reply::json(200, &page2));
    server.push(Reply::json(200, &page1));
    let p = provider(&server, key());
    let models = p.list_models().await.unwrap();
    assert_eq!(
        models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["claude-new-9", MODEL]
    );
    let reqs = server.requests();
    assert_eq!(reqs[0].method, "GET");
    assert_eq!(reqs[0].path, "/v1/models?limit=100");
    assert_eq!(reqs[1].path, "/v1/models?limit=100&after_id=claude-new-9");
    assert_eq!(reqs[1].header("anthropic-version"), Some("2023-06-01"));
    let caps = p
        .capabilities()
        .models
        .get("claude-new-9")
        .cloned()
        .unwrap();
    assert_eq!(caps.thinking, ThinkingSupport::Optional);
    assert_eq!(caps.context_window, Some(500_000));
    assert!(caps.effort && caps.vision && caps.strict_tools);

    server.reset(http_error(
        403,
        None,
        &sse::error_body("permission_error", "nope"),
    ));
    assert_eq!(
        p.list_models().await.unwrap_err().kind,
        ProviderErrorKind::Auth
    );
}

#[tokio::test]
async fn cancel_closes_connection_within_100ms() {
    let server = FixtureServer::start().await;
    let prefix = sse::message_start(MODEL)
        + &sse::event(
            "content_block_start",
            &json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        );
    let deltas = (0..40)
        .map(|i| sse::event("content_block_delta", &json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": format!("s{i} ")}})))
        .collect();
    server.reset(slow(
        prefix,
        deltas,
        Duration::from_millis(50),
        String::new(),
    ));
    let p = provider(&server, key());
    let cancel = CancellationToken::new();
    let mut stream = p.stream(req(), cancel.clone());
    while let Some(ev) = stream.next().await {
        if matches!(ev, ProviderEvent::TextDelta { .. }) {
            break;
        }
    }
    let at = Instant::now();
    cancel.cancel();
    // Bez odpytywania strumienia: zadanie w tle zrywa połączenie samo.
    let mut waited = Duration::ZERO;
    while server.disconnects().is_empty() && waited < Duration::from_secs(1) {
        tokio::time::sleep(Duration::from_millis(5)).await;
        waited += Duration::from_millis(5);
    }
    let closed = server
        .disconnects()
        .first()
        .copied()
        .expect("serwer nie zauważył rozłączenia");
    assert!(
        closed.duration_since(at) < Duration::from_millis(100),
        "zerwanie po {:?}",
        closed.duration_since(at)
    );
    let rest: Vec<_> = stream.collect().await;
    assert_eq!(
        rest.last(),
        Some(&ProviderEvent::stop(StopReason::Cancelled))
    );

    // Upuszczenie strumienia też zrywa połączenie.
    server.reset(Reply::sse_parts(vec![
        Part::Bytes(sse::message_start(MODEL).into_bytes()),
        Part::Hang,
    ]));
    let mut stream = p.stream(req(), CancellationToken::new());
    let _ = stream.next().await;
    let before = server.disconnects().len();
    drop(stream);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.disconnects().len(), before + 1);
}
