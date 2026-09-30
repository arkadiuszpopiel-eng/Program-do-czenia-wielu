//! Adapter OpenAI Chat Completions (natywny i „zgodny") na nagraniach SSE.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use futures_util::StreamExt;
use providers_api_impl::{AuthScheme, OpenAiOptions, OpenAiProvider, ProviderProfile};
use providers_contract::contract_tests::{self, Scenario};
use providers_contract::{
    CancellationToken, ChatRequest, ContentBlock, Effort, EmbeddingRequest, ImageSource, Message,
    ModelCapabilities, ModelKind, ModelProvider, ProviderErrorKind, ProviderEvent, Role, StaticKey,
    StopReason, ThinkingSupport, ToolChoice, ToolResult, ToolResultPart, ToolSpec, ToolUse,
    TurnAccumulator,
};
use serde_json::json;
use support::openai_sse as sse;
use support::{FixtureServer, Part, Reply, WireHarness, http, http_error, profile, slow};

const MODEL: &str = "gpt-6-luna";

fn error_body(status: u16) -> serde_json::Value {
    match status {
        401 => sse::error_body(
            "invalid_request_error",
            "Incorrect API key",
            Some("invalid_api_key"),
        ),
        429 => sse::error_body(
            "requests",
            "Rate limit reached",
            Some("rate_limit_exceeded"),
        ),
        400 | 404 => sse::error_body("invalid_request_error", "bad", None),
        _ => sse::error_body("server_error", "The server had an error", None),
    }
}

fn reply(s: &Scenario) -> Option<Reply> {
    Some(match s {
        Scenario::Text { chunks } => {
            let c: Vec<&str> = chunks.iter().map(String::as_str).collect();
            Reply::sse(sse::chat_text(MODEL, &c, "stop"))
        }
        Scenario::ToolCall {
            id,
            name,
            arguments,
        } => Reply::sse(sse::chat_tool(MODEL, id, name, &arguments.to_string())),
        Scenario::Thinking { .. } => return None, // Chat Completions nie ma podpisanego myślenia
        Scenario::HttpError {
            status,
            retry_after_s,
        } => http_error(*status, *retry_after_s, &error_body(*status)),
        Scenario::Refusal => Reply::sse(sse::chat_text(MODEL, &[], "content_filter")),
        Scenario::MaxTokens { text } => Reply::sse(sse::chat_text(MODEL, &[text], "length")),
        Scenario::Stall => Reply::sse_parts(vec![Part::Hang]),
        Scenario::Slow { chunks, interval } => slow(
            sse::chat_role(MODEL),
            chunks
                .iter()
                .map(|c| sse::chat_text_delta(MODEL, c))
                .collect(),
            *interval,
            sse::chat_finish(MODEL, "stop"),
        ),
    })
}

fn tools_caps() -> ModelCapabilities {
    ModelCapabilities {
        tools: true,
        strict_tools: true,
        forced_tool_choice: true,
        vision: true,
        ..ModelCapabilities::default()
    }
}

fn make(options: OpenAiOptions) -> impl Fn(&FixtureServer, ProviderProfile) -> OpenAiProvider {
    move |server, profile| {
        OpenAiProvider::new(
            profile,
            http(server, AuthScheme::Bearer),
            Arc::new(StaticKey::new("sk-oai-test")),
            options.clone(),
        )
        .unwrap()
    }
}

#[tokio::test]
async fn contract_suite_native_chat() {
    let h = WireHarness::new(
        "openai",
        MODEL,
        Box::new(reply),
        Box::new(make(OpenAiOptions::native_chat())),
    )
    .await;
    contract_tests::run_all(&h).await;
}

#[tokio::test]
async fn contract_suite_openai_compatible() {
    let mut h = WireHarness::new(
        "xai",
        MODEL,
        Box::new(reply),
        Box::new(make(OpenAiOptions::compatible())),
    )
    .await;
    h.caps = Some(tools_caps());
    contract_tests::run_all(&h).await;
}

fn provider(
    server: &FixtureServer,
    options: OpenAiOptions,
    caps: Option<ModelCapabilities>,
) -> OpenAiProvider {
    let mut p = profile("openai", MODEL);
    if let Some(c) = caps {
        p.models.insert(MODEL.into(), c);
    }
    make(options)(server, p)
}

async fn run(p: &OpenAiProvider, req: ChatRequest) -> Vec<ProviderEvent> {
    p.stream(req, CancellationToken::new()).collect().await
}

fn history() -> Vec<Message> {
    let image = ImageSource::Base64 {
        media_type: "image/png".into(),
        data: "iVBOR".into(),
    };
    vec![
        Message::new(
            Role::User,
            vec![
                ContentBlock::text("Co widać?"),
                ContentBlock::Image { source: image },
            ],
        ),
        Message::new(
            Role::Assistant,
            vec![
                ContentBlock::text("Sprawdzę."),
                ContentBlock::ToolUse(ToolUse {
                    id: "call_9".into(),
                    name: "ocr".into(),
                    input: json!({"lang": "pl"}),
                }),
            ],
        ),
        Message::new(
            Role::User,
            vec![ContentBlock::ToolResult(ToolResult {
                tool_use_id: "call_9".into(),
                content: vec![ToolResultPart::Text {
                    text: "Faktura".into(),
                }],
                is_error: false,
            })],
        ),
    ]
}

fn ocr_tool() -> ToolSpec {
    ToolSpec {
        name: "ocr".into(),
        description: "OCR".into(),
        input_schema: json!({"type": "object"}),
        strict: true,
    }
}

#[tokio::test]
async fn native_chat_request_shape() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(sse::chat_text(MODEL, &["ok"], "stop")));
    let caps = ModelCapabilities {
        effort: true,
        ..tools_caps()
    };
    let p = provider(&server, OpenAiOptions::native_chat(), Some(caps));
    let mut req = ChatRequest::new(MODEL, history())
        .with_system("Jesteś Alfą.")
        .with_tool(ocr_tool());
    req.tool_choice = ToolChoice::Required;
    req.params.temperature = Some(0.5);
    req.params.effort = Some(Effort::Max);
    run(&p, req).await;
    let rec = server.requests().pop().unwrap();
    assert_eq!(rec.path, "/chat/completions");
    assert_eq!(rec.header("authorization"), Some("Bearer sk-oai-test"));
    let body = rec.json();
    assert_eq!(body["stream_options"], json!({"include_usage": true}));
    assert_eq!(body["max_completion_tokens"], 8_192);
    assert_eq!(body["tool_choice"], "required");
    assert_eq!(
        body["tools"][0],
        json!({"type": "function", "function": {"name": "ocr", "description": "OCR", "parameters": {"type": "object"}, "strict": true}})
    );
    assert_eq!(body["reasoning_effort"], "high");
    assert!(
        body.get("temperature").is_none(),
        "bez próbkowania w możliwościach"
    );
    let msgs = body["messages"].as_array().unwrap();
    assert_eq!(
        msgs[0],
        json!({"role": "system", "content": "Jesteś Alfą."})
    );
    assert_eq!(
        msgs[1]["content"][1],
        json!({"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBOR"}})
    );
    assert_eq!(
        msgs[2]["tool_calls"][0]["function"],
        json!({"name": "ocr", "arguments": "{\"lang\":\"pl\"}"})
    );
    assert_eq!(
        msgs[3],
        json!({"role": "tool", "tool_call_id": "call_9", "content": "Faktura"})
    );
}

#[tokio::test]
async fn compatible_uses_max_tokens_and_needs_declared_tools() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(sse::chat_text(MODEL, &["ok"], "stop")));
    let p = provider(&server, OpenAiOptions::compatible(), None);
    let events = run(
        &p,
        ChatRequest::new(MODEL, vec![Message::user_text("x")]).with_tool(ocr_tool()),
    )
    .await;
    assert!(
        matches!(events.as_slice(), [ProviderEvent::Error(e)] if e.kind == ProviderErrorKind::Unsupported)
    );
    let caps = ModelCapabilities {
        sampling: true,
        ..tools_caps()
    };
    let p = provider(&server, OpenAiOptions::compatible(), Some(caps));
    let mut req = ChatRequest::new(MODEL, vec![Message::user_text("x")]);
    req.params.temperature = Some(0.7);
    req.params.stop = vec!["KONIEC".into()];
    run(&p, req).await;
    let body = server.requests().pop().unwrap().json();
    assert_eq!(body["max_tokens"], 8_192);
    assert_eq!(body["messages"][0], json!({"role": "user", "content": "x"}));
    assert!((body["temperature"].as_f64().unwrap() - 0.7).abs() < 1e-6);
    assert_eq!(body["stop"], json!(["KONIEC"]));
    assert!(body.get("reasoning_effort").is_none());
}

#[tokio::test]
async fn usage_reasoning_and_parallel_tools() {
    let server = FixtureServer::start().await;
    let body = sse::chat_role(MODEL)
        + &sse::chat_reasoning_delta(MODEL, "Myślę…")
        + &sse::data(
            &json!({"id": "c", "model": MODEL, "choices": [{"index": 0, "delta": {"tool_calls": [
            {"index": 0, "id": "a", "type": "function", "function": {"name": "t1", "arguments": "{}"}},
            {"index": 1, "id": "b", "type": "function", "function": {"name": "t2", "arguments": "{\"x\":1}"}}]}, "finish_reason": null}]}),
        )
        + &sse::chat_finish(MODEL, "stop");
    server.reset(Reply::sse(body));
    let p = provider(&server, OpenAiOptions::compatible(), Some(tools_caps()));
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::ToolUse)),
        "`stop` z narzędziami → ToolUse"
    );
    let mut acc = TurnAccumulator::new(p.id().clone());
    events.iter().for_each(|e| acc.push(e));
    let turn = acc.finish();
    assert_eq!(turn.usage.input_tokens, 20);
    assert_eq!(turn.usage.cache_read_tokens, 100);
    assert_eq!(turn.message.tool_uses().count(), 2);
    assert!(
        matches!(&turn.message.content[0], ContentBlock::Thinking(t) if t.text == "Myślę…" && t.signature.is_none())
    );

    // Myślenie bez podpisu nie wraca do dostawcy.
    let mut next = ChatRequest::new(MODEL, vec![Message::user_text("x"), turn.message]);
    next.messages.push(Message::user_text("dalej"));
    server.reset(Reply::sse(sse::chat_text(MODEL, &["ok"], "stop")));
    run(&p, next).await;
    assert!(!server.requests().pop().unwrap().body.contains("Myślę"));
}

#[tokio::test]
async fn stream_edge_cases_and_errors() {
    let server = FixtureServer::start().await;
    let p = provider(&server, OpenAiOptions::compatible(), None);
    // Bez `[DONE]`, ale z finish_reason.
    server.reset(Reply::sse(sse::chat_role(MODEL) + &sse::chat_text_delta(MODEL, "a") + &sse::data(&json!({"model": MODEL, "choices": [{"index": 0, "delta": {}, "finish_reason": "length"}]}))));
    assert_eq!(
        run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")]))
            .await
            .last(),
        Some(&ProviderEvent::stop(StopReason::MaxTokens))
    );
    // Błąd w strumieniu.
    server.reset(Reply::sse(
        sse::chat_role(MODEL)
            + &sse::data(&json!({"error": {"message": "overloaded", "type": "server_overloaded"}})),
    ));
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    assert!(
        matches!(events.last(), Some(ProviderEvent::Error(e)) if matches!(e.kind, ProviderErrorKind::Overloaded { .. }))
    );
    // 429 insufficient_quota = Auth, bez ponawiania.
    server.reset(http_error(
        429,
        Some(1),
        &sse::error_body("insufficient_quota", "quota", Some("insufficient_quota")),
    ));
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    assert!(
        matches!(events.as_slice(), [ProviderEvent::Error(e)] if e.kind == ProviderErrorKind::Auth)
    );
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn models_and_embeddings() {
    let server = FixtureServer::start().await;
    server.reset(Reply::json(
        200,
        &json!({"object": "list", "data": [
        {"id": MODEL, "object": "model", "created": 1_790_000_000, "owned_by": "openai"},
        {"id": "text-embedding-4", "object": "model", "created": 1, "owned_by": "openai"}]}),
    ));
    let emb_caps = ModelCapabilities {
        kinds: vec![ModelKind::Embeddings],
        ..ModelCapabilities::default()
    };
    let mut prof = profile("openai", MODEL);
    prof.models.insert("text-embedding-4".into(), emb_caps);
    prof.models.insert(
        MODEL.into(),
        ModelCapabilities {
            thinking: ThinkingSupport::AlwaysOn,
            ..tools_caps()
        },
    );
    let p = make(OpenAiOptions::native())(&server, prof);
    let models = p.list_models().await.unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].created.as_deref(), Some("1790000000"));
    assert_eq!(server.requests()[0].path, "/models");

    server.reset(Reply::json(200, &json!({"data": [
        {"index": 1, "embedding": [0.5, 0.5]}, {"index": 0, "embedding": [1.0, 0.0]}], "usage": {"prompt_tokens": 6}})));
    let out = p
        .embed(EmbeddingRequest {
            model: "text-embedding-4".into(),
            input: vec!["a".into(), "b".into()],
        })
        .await
        .unwrap();
    assert_eq!(out.vectors, vec![vec![1.0, 0.0], vec![0.5, 0.5]]);
    assert_eq!(out.usage.input_tokens, 6);
    let err = p
        .embed(EmbeddingRequest {
            model: MODEL.into(),
            input: vec!["a".into()],
        })
        .await
        .unwrap_err();
    assert_eq!(err.kind, ProviderErrorKind::Unsupported);
}
