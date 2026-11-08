//! Adapter Anthropic na nagraniach SSE: zestaw kontraktowy + kształt żądania i odpowiedzi.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use futures_util::StreamExt;
use providers_api_impl::anthropic::{BETA_SERVER_FALLBACK, MID_SYSTEM_PREFIX};
use providers_api_impl::{AnthropicOptions, AnthropicProvider, AuthScheme};
use providers_contract::contract_tests::{self, Scenario};
use providers_contract::{
    CancellationToken, ChatRequest, ContentBlock, Effort, Message, ModelProvider,
    ProviderErrorKind, ProviderEvent, ProviderOrigin, Role, StaticKey, StopReason, ThinkingBlock,
    ThinkingDisplay, ToolChoice, ToolSpec, TurnAccumulator,
};
use serde_json::json;
use support::anthropic_sse::{self as sse, Block};
use support::{FixtureServer, Reply, WireHarness, http, http_error, profile, slow};

const MODEL: &str = "claude-opus-5-5";

fn reply(s: &Scenario) -> Option<Reply> {
    Some(match s {
        Scenario::Text { chunks } => {
            let chunks: Vec<&str> = chunks.iter().map(String::as_str).collect();
            Reply::sse(sse::text(MODEL, &chunks))
        }
        Scenario::ToolCall {
            id,
            name,
            arguments,
        } => {
            let raw = arguments.to_string();
            let (a, b) = raw.split_at(raw.len() / 2);
            let tool = Block::Tool {
                id: id.clone(),
                name: name.clone(),
                parts: vec![a.into(), b.into()],
            };
            Reply::sse(sse::response(MODEL, &[tool], "tool_use", None))
        }
        Scenario::Thinking {
            thinking,
            signature,
            text,
        } => Reply::sse(sse::response(
            MODEL,
            &[
                Block::Thinking {
                    text: thinking.clone(),
                    signature: signature.clone(),
                },
                Block::Text(vec![text.clone()]),
            ],
            "end_turn",
            None,
        )),
        Scenario::HttpError {
            status,
            retry_after_s,
        } => http_error(
            *status,
            *retry_after_s,
            &sse::error_body(sse::error_type_for(*status), "fixture"),
        ),
        Scenario::Refusal => Reply::sse(sse::response(
            MODEL,
            &[],
            "refusal",
            Some(json!({"type": "refusal", "category": "cyber", "explanation": null})),
        )),
        Scenario::MaxTokens { text } => Reply::sse(sse::response(
            MODEL,
            &[Block::Text(vec![text.clone()])],
            "max_tokens",
            None,
        )),
        Scenario::Stall => Reply::sse_parts(vec![support::Part::Hang]),
        Scenario::Slow { chunks, interval } => {
            let prefix = sse::message_start(MODEL)
                + &sse::event(
                    "content_block_start",
                    &json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
                );
            let deltas = chunks
                .iter()
                .map(|c| sse::event("content_block_delta", &json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": c}})))
                .collect();
            let suffix = sse::event(
                "content_block_stop",
                &json!({"type": "content_block_stop", "index": 0}),
            ) + &sse::finish("end_turn", None, 9);
            slow(prefix, deltas, *interval, suffix)
        }
    })
}

fn make(
    options: AnthropicOptions,
) -> impl Fn(&FixtureServer, providers_api_impl::ProviderProfile) -> AnthropicProvider {
    move |server, profile| {
        AnthropicProvider::new(
            profile,
            http(server, AuthScheme::XApiKey),
            Arc::new(StaticKey::new("sk-ant-test-123")),
            options.clone(),
        )
        .unwrap()
    }
}

async fn harness(options: AnthropicOptions) -> WireHarness<AnthropicProvider> {
    WireHarness::new("anthropic", MODEL, Box::new(reply), Box::new(make(options))).await
}

#[tokio::test]
async fn contract_suite_native() {
    contract_tests::run_all(&harness(AnthropicOptions::native()).await).await;
}

#[tokio::test]
async fn contract_suite_compatible_endpoint() {
    let mut h = harness(AnthropicOptions::compatible()).await;
    h.id = "custom-anthropic-compatible".into();
    contract_tests::run_all(&h).await;
}

async fn provider(server: &FixtureServer, options: AnthropicOptions) -> AnthropicProvider {
    make(options)(server, profile("anthropic", MODEL))
}

async fn run(p: &AnthropicProvider, req: ChatRequest) -> Vec<ProviderEvent> {
    p.stream(req, CancellationToken::new()).collect().await
}

fn tool(name: &str) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: format!("narzędzie {name}"),
        input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false, "required": []}),
        strict: true,
    }
}

#[tokio::test]
async fn opus_5_5_request_shape_follows_api_rules() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(sse::text(MODEL, &["ok"])));
    let p = provider(&server, AnthropicOptions::native()).await;
    let mut req = ChatRequest::new(MODEL, vec![Message::user_text("Która godzina?")])
        .with_system("Jesteś Alfą.")
        .with_tool(tool("zegar"))
        .with_tool(tool("alarm"));
    req.tool_choice = ToolChoice::Tool {
        name: "zegar".into(),
    };
    req.params.thinking.enabled = false; // nie da się wyłączyć na Opus 5.5
    req.params.temperature = Some(0.3); // Opus 5.5: temperature → 400, adapter pomija
    let events = run(&p, req).await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );

    let rec = server.requests().pop().unwrap();
    assert_eq!(rec.path, "/v1/messages");
    assert_eq!(rec.header("x-api-key"), Some("sk-ant-test-123"));
    assert_eq!(rec.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(rec.header("accept"), Some("text/event-stream"));
    assert!(
        rec.header("anthropic-beta")
            .unwrap()
            .contains(BETA_SERVER_FALLBACK)
    );
    let body = rec.json();
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 128_000);
    assert_eq!(body["thinking"], json!({"type": "adaptive"}));
    assert_eq!(
        body["output_config"],
        json!({"effort": "medium"}),
        "effort jawnie, domyślnie medium"
    );
    assert!(body.get("temperature").is_none());
    assert_eq!(
        body["tool_choice"],
        json!({"type": "auto"}),
        "wymuszony tool_choice → auto na Opus 5.5"
    );
    assert_eq!(
        body["tools"][0]["name"], "alarm",
        "narzędzia posortowane (stabilny prefiks)"
    );
    assert_eq!(body["tools"][1]["strict"], true);
    assert_eq!(body["tools"][1]["eager_input_streaming"], true);
    assert_eq!(
        body["system"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
    assert!(
        body["tools"][1].get("cache_control").is_none(),
        "system niesie punkt cache"
    );
    assert_eq!(
        body["messages"][0]["content"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
    assert_eq!(body["fallbacks"], "default");
}

#[tokio::test]
async fn optional_thinking_model_can_disable_and_force_tools() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(sse::text("claude-opus-5", &["ok"])));
    let p = provider(&server, AnthropicOptions::native()).await;
    let mut req =
        ChatRequest::new("claude-opus-5", vec![Message::user_text("x")]).with_tool(tool("zegar"));
    req.tool_choice = ToolChoice::Required;
    req.params.thinking.enabled = false;
    req.params.effort = Some(Effort::XHigh);
    req.cache.enabled = false;
    run(&p, req.clone()).await;
    let body = server.requests().pop().unwrap().json();
    assert_eq!(body["thinking"], json!({"type": "disabled"}));
    assert_eq!(body["tool_choice"], json!({"type": "any"}));
    assert_eq!(body["output_config"]["effort"], "xhigh");
    assert!(body.to_string().find("cache_control").is_none());

    req.params.thinking.enabled = true;
    req.params.thinking.display = ThinkingDisplay::Summarized;
    run(&p, req).await;
    let body = server.requests().pop().unwrap().json();
    assert_eq!(
        body["thinking"],
        json!({"type": "adaptive", "display": "summarized"})
    );
}

#[tokio::test]
async fn compatible_endpoint_sends_only_core_api() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(sse::text("glm-x", &["ok"])));
    let p = provider(&server, AnthropicOptions::compatible()).await;
    let mut req = ChatRequest::new("glm-x", vec![Message::user_text("x")]).with_tool(tool("zegar"));
    req.params.thinking.display = ThinkingDisplay::Updates;
    let events = run(&p, req).await;
    assert!(
        matches!(events.last(), Some(ProviderEvent::Error(e)) if e.kind == ProviderErrorKind::Unsupported),
        "nieznany model endpointu zgodnego: ostrożnie bez narzędzi"
    );
    let events = run(&p, ChatRequest::new("glm-x", vec![Message::user_text("x")])).await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
    let rec = server.requests().pop().unwrap();
    assert!(rec.header("anthropic-beta").is_none());
    let body = rec.json();
    for absent in ["thinking", "output_config", "fallbacks", "temperature"] {
        assert!(
            body.get(absent).is_none(),
            "{absent} nie powinno być wysłane: {body}"
        );
    }
}

fn thinking(provider: &str, signature: Option<&str>) -> ContentBlock {
    ContentBlock::Thinking(ThinkingBlock {
        text: String::new(),
        signature: signature.map(str::to_owned),
        provider_origin: ProviderOrigin {
            provider: provider.into(),
            model: MODEL.into(),
        },
    })
}

#[tokio::test]
async fn history_rendering_keeps_own_thinking_and_drops_foreign() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(sse::text(MODEL, &["ok"])));
    let p = provider(&server, AnthropicOptions::native()).await;
    let assistant = Message::new(
        Role::Assistant,
        vec![
            thinking("anthropic", Some("sig-own")),
            thinking("openai", Some("sig-foreign")),
            thinking("anthropic", None),
            ContentBlock::text("Odpowiedź."),
        ],
    );
    let req = ChatRequest::new(
        MODEL,
        vec![
            Message::user_text("a"),
            assistant,
            Message::system_text("Mów krócej."),
            Message::user_text("b"),
        ],
    );
    run(&p, req).await;
    let body = server.requests().pop().unwrap().json();
    let msgs = body["messages"].as_array().unwrap();
    assert_eq!(
        msgs[1]["content"].as_array().unwrap().len(),
        2,
        "własny podpisany + tekst"
    );
    assert_eq!(
        msgs[1]["content"][0],
        json!({"type": "thinking", "thinking": "", "signature": "sig-own"})
    );
    assert_eq!(msgs[2]["role"], "user");
    assert_eq!(
        msgs[2]["content"][0]["text"],
        format!("{MID_SYSTEM_PREFIX}Mów krócej.")
    );
}

#[tokio::test]
async fn refusal_details_and_server_fallback_switch() {
    let server = FixtureServer::start().await;
    let body = sse::response(
        MODEL,
        &[
            Block::Thinking {
                text: String::new(),
                signature: "sig-a".into(),
            },
            Block::Text(vec!["Zaczynam ".into()]),
            Block::Fallback {
                from: MODEL.into(),
                to: "claude-opus-5".into(),
            },
            Block::Text(vec!["i kończę.".into()]),
        ],
        "end_turn",
        None,
    );
    server.reset(Reply::sse(body));
    let p = provider(&server, AnthropicOptions::native()).await;
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    assert!(events.contains(&ProviderEvent::ModelSwitched {
        from: MODEL.into(),
        to: "claude-opus-5".into()
    }));
    let mut acc = TurnAccumulator::new(p.id().clone());
    events.iter().for_each(|e| acc.push(e));
    let turn = acc.finish();
    assert_eq!(turn.message.visible_text(), "Zaczynam i kończę.");
    assert!(
        !turn.message.content.iter().any(ContentBlock::is_thinking),
        "myślenie sprzed przełączenia odpada"
    );
    assert_eq!(turn.usage.cache_read_tokens, 100);
    assert_eq!(turn.usage.cache_write_tokens, 7);
    assert_eq!(turn.usage.output_tokens, 42);

    server.reset(Reply::sse(sse::response(
        MODEL,
        &[],
        "refusal",
        Some(json!({"category": "bio", "explanation": "x"})),
    )));
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    match events.last() {
        Some(ProviderEvent::Stop {
            reason: StopReason::Refusal,
            details: Some(d),
        }) => {
            assert_eq!(d.category.as_deref(), Some("bio"));
        }
        other => panic!("{other:?}"),
    }
}
