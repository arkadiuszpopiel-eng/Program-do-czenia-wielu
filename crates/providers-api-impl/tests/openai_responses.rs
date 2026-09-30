//! Adapter OpenAI Responses API na nagraniach SSE (`response.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use futures_util::StreamExt;
use providers_api_impl::{AuthScheme, OpenAiOptions, OpenAiProvider, ProviderProfile};
use providers_contract::contract_tests::{self, Scenario};
use providers_contract::{
    CancellationToken, ChatRequest, ContentBlock, Message, ModelCapabilities, ModelProvider,
    ProviderErrorKind, ProviderEvent, Role, StaticKey, StopReason, ThinkingDisplay,
    ThinkingSupport, ToolResult, ToolResultPart, ToolSpec, ToolUse, TurnAccumulator,
};
use serde_json::json;
use support::openai_sse as sse;
use support::{FixtureServer, Part, Reply, WireHarness, http, http_error, profile, slow};

const MODEL: &str = "gpt-6-sol";

fn caps() -> ModelCapabilities {
    ModelCapabilities {
        tools: true,
        strict_tools: true,
        forced_tool_choice: true,
        thinking: ThinkingSupport::AlwaysOn,
        effort: true,
        ..ModelCapabilities::default()
    }
}

fn reply(s: &Scenario) -> Option<Reply> {
    Some(match s {
        Scenario::Text { chunks } => {
            let c: Vec<&str> = chunks.iter().map(String::as_str).collect();
            Reply::sse(sse::resp_text(MODEL, &c))
        }
        Scenario::ToolCall {
            id,
            name,
            arguments,
        } => Reply::sse(sse::resp_tool(MODEL, id, name, &arguments.to_string())),
        Scenario::Thinking {
            thinking,
            signature,
            text,
        } => Reply::sse(
            sse::resp_created(MODEL)
                + &sse::resp_reasoning(thinking, signature)
                + &sse::resp_message_added(1)
                + &sse::resp_text_delta(1, text)
                + &sse::resp_completed(MODEL),
        ),
        Scenario::HttpError {
            status,
            retry_after_s,
        } => {
            let body = match status {
                429 => sse::error_body("requests", "Rate limit", Some("rate_limit_exceeded")),
                400 | 401 => sse::error_body("invalid_request_error", "bad", None),
                _ => sse::error_body("server_error", "boom", None),
            };
            http_error(*status, *retry_after_s, &body)
        }
        Scenario::Refusal => Reply::sse(sse::resp_refusal(MODEL)),
        Scenario::MaxTokens { text } => Reply::sse(
            sse::resp_created(MODEL)
                + &sse::resp_message_added(0)
                + &sse::resp_text_delta(0, text)
                + &sse::resp_incomplete(MODEL, "max_output_tokens"),
        ),
        Scenario::Stall => Reply::sse_parts(vec![Part::Hang]),
        Scenario::Slow { chunks, interval } => slow(
            sse::resp_created(MODEL) + &sse::resp_message_added(0),
            chunks.iter().map(|c| sse::resp_text_delta(0, c)).collect(),
            *interval,
            sse::resp_completed(MODEL),
        ),
    })
}

fn make(server: &FixtureServer, profile: ProviderProfile) -> OpenAiProvider {
    OpenAiProvider::new(
        profile,
        http(server, AuthScheme::Bearer),
        Arc::new(StaticKey::new("sk-oai")),
        OpenAiOptions::native(),
    )
    .unwrap()
}

#[tokio::test]
async fn contract_suite_responses_api() {
    let mut h = WireHarness::new("openai", MODEL, Box::new(reply), Box::new(make)).await;
    h.caps = Some(caps());
    contract_tests::run_all(&h).await;
}

fn provider(server: &FixtureServer) -> OpenAiProvider {
    let mut p = profile("openai", MODEL);
    p.models.insert(MODEL.into(), caps());
    make(server, p)
}

async fn run(p: &OpenAiProvider, req: ChatRequest) -> Vec<ProviderEvent> {
    p.stream(req, CancellationToken::new()).collect().await
}

#[tokio::test]
async fn reasoning_round_trips_as_encrypted_item() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(
        sse::resp_created(MODEL)
            + &sse::resp_reasoning("Plan", "gAAAAB-enc")
            + &sse::resp_message_added(1)
            + &sse::resp_text_delta(1, "Gotowe")
            + &sse::resp_completed(MODEL),
    ));
    let p = provider(&server);
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    let mut acc = TurnAccumulator::new(p.id().clone());
    events.iter().for_each(|e| acc.push(e));
    let turn = acc.finish();
    assert_eq!(turn.usage.input_tokens, 20);
    match &turn.message.content[0] {
        ContentBlock::Thinking(t) => {
            assert_eq!(t.signature.as_deref(), Some("rs_fixture:gAAAAB-enc"))
        }
        other => panic!("{other:?}"),
    }
    let tool_use = Message::new(
        Role::Assistant,
        vec![ContentBlock::ToolUse(ToolUse {
            id: "call_1".into(),
            name: "t".into(),
            input: json!({"a": 1}),
        })],
    );
    let result = Message::new(
        Role::User,
        vec![ContentBlock::ToolResult(ToolResult {
            tool_use_id: "call_1".into(),
            content: vec![ToolResultPart::Text {
                text: "wynik".into(),
            }],
            is_error: true,
        })],
    );
    let mut req = ChatRequest::new(
        MODEL,
        vec![Message::user_text("x"), turn.message, tool_use, result],
    )
    .with_system("Instrukcje")
    .with_tool(ToolSpec {
        name: "t".into(),
        description: "d".into(),
        input_schema: json!({"type": "object"}),
        strict: false,
    });
    req.params.thinking.display = ThinkingDisplay::Summarized;
    run(&p, req).await;
    let rec = server.requests().pop().unwrap();
    assert_eq!(rec.path, "/responses");
    let body = rec.json();
    assert_eq!(body["store"], false);
    assert_eq!(body["instructions"], "Instrukcje");
    assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(
        body["reasoning"],
        json!({"effort": "medium", "summary": "auto"})
    );
    assert_eq!(
        body["tools"][0]["strict"], false,
        "Responses domyślnie strict — wysyłamy jawnie"
    );
    let input = body["input"].as_array().unwrap();
    assert_eq!(
        input[1],
        json!({"type": "reasoning", "id": "rs_fixture", "encrypted_content": "gAAAAB-enc",
                                "summary": [{"type": "summary_text", "text": "Plan"}]})
    );
    assert_eq!(
        input[2],
        json!({"role": "assistant", "content": [{"type": "output_text", "text": "Gotowe"}]})
    );
    assert_eq!(
        input[3],
        json!({"type": "function_call", "call_id": "call_1", "name": "t", "arguments": "{\"a\":1}"})
    );
    assert_eq!(
        input[4],
        json!({"type": "function_call_output", "call_id": "call_1", "output": "BŁĄD: wynik"})
    );
}

#[tokio::test]
async fn failure_and_incomplete_statuses() {
    let server = FixtureServer::start().await;
    let p = provider(&server);
    server.reset(Reply::sse(
        sse::resp_created(MODEL)
            + &sse::ev(
                "response.failed",
                json!({"response": {"id": "r", "status": "failed",
        "error": {"code": "rate_limit_exceeded", "message": "slow"}}}),
            ),
    ));
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    assert!(
        matches!(events.last(), Some(ProviderEvent::Error(e)) if matches!(e.kind, ProviderErrorKind::RateLimited { .. }))
    );
    server.reset(Reply::sse(
        sse::resp_created(MODEL) + &sse::resp_incomplete(MODEL, "content_filter"),
    ));
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::Refusal))
    );
    server.reset(Reply::sse(
        sse::resp_created(MODEL)
            + &sse::ev("error", json!({"code": "server_error", "message": "x"})),
    ));
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    assert!(
        matches!(events.last(), Some(ProviderEvent::Error(e)) if matches!(e.kind, ProviderErrorKind::Server { .. }))
    );
    server.reset(Reply::sse(sse::resp_created(MODEL)));
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    assert!(
        matches!(events.last(), Some(ProviderEvent::Error(e)) if e.kind == ProviderErrorKind::Protocol)
    );
}
