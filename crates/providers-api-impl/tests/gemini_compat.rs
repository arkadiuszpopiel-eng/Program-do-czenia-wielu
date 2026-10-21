//! Przegląd 2026-10-22: Gemini przez endpoint zgodny z OpenAI — równoległe wywołania narzędzi bez
//! pola `index` nie sklejają się w jedno, a identyfikatory `models/…` trafiają w cennik.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use futures_util::StreamExt;
use providers_api_impl::{AccountProfile, CatalogEntry, build_provider};
use providers_contract::{
    CancellationToken, ChatRequest, Message, ModelProvider, Pricing, ProviderEvent, StaticKey,
    StopReason, ToolSpec, TurnAccumulator, Usage,
};
use serde_json::json;
use support::openai_sse as sse;
use support::{FixtureServer, Reply};

const MODEL: &str = "gemini-2.5-pro";

fn provider(server: &FixtureServer) -> Arc<dyn ModelProvider> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../providers-catalog/google.toml");
    let entry = CatalogEntry::parse_toml(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut account = AccountProfile::new(Arc::new(StaticKey::new("key-123")));
    account.base_url = Some(server.url());
    account.default_model = Some(MODEL.into());
    account.pricing.insert(
        MODEL.into(),
        Pricing {
            input_per_mtok_usd: 1.25,
            output_per_mtok_usd: 10.0,
            cache_read_per_mtok_usd: None,
            cache_write_per_mtok_usd: None,
        },
    );
    build_provider(&entry, account).unwrap()
}

#[tokio::test]
async fn parallel_tool_calls_without_index_stay_separate() {
    let server = FixtureServer::start().await;
    let calls = json!({"id": "c", "model": MODEL, "choices": [{"index": 0, "delta": {"tool_calls": [
        {"id": "a", "type": "function", "function": {"name": "fs_read", "arguments": "{\"path\":\"a.txt\"}"}},
        {"id": "b", "type": "function", "function": {"name": "fs_read", "arguments": "{\"path\":\"b.txt\"}"}}
    ]}, "finish_reason": null}]});
    server.reset(Reply::sse(
        sse::chat_role(MODEL) + &sse::data(&calls) + &sse::chat_finish(MODEL, "stop"),
    ));
    let p = provider(&server);
    let mut req = ChatRequest::new(MODEL, vec![Message::user_text("przeczytaj oba")]);
    req.tools = vec![ToolSpec {
        name: "fs_read".into(),
        description: "Czyta plik.".into(),
        input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        strict: false,
    }];
    let events: Vec<ProviderEvent> = p.stream(req, CancellationToken::new()).collect().await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::ToolUse))
    );
    let mut acc = TurnAccumulator::new(p.id().clone());
    events.iter().for_each(|e| acc.push(e));
    let turn = acc.finish();
    let paths: Vec<String> = turn
        .message
        .tool_uses()
        .map(|t| t.input["path"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(paths, ["a.txt", "b.txt"]);
}

#[test]
fn models_prefix_hits_the_price_list() {
    let server_url = "http://127.0.0.1:9".to_owned();
    let entry = {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../providers-catalog/google.toml");
        CatalogEntry::parse_toml(&std::fs::read_to_string(path).unwrap()).unwrap()
    };
    let mut account = AccountProfile::new(Arc::new(StaticKey::new("key-123")));
    account.base_url = Some(server_url);
    account.pricing.insert(
        MODEL.into(),
        Pricing {
            input_per_mtok_usd: 1.25,
            output_per_mtok_usd: 10.0,
            cache_read_per_mtok_usd: None,
            cache_write_per_mtok_usd: None,
        },
    );
    let p = build_provider(&entry, account).unwrap();
    let usage = Usage {
        input_tokens: 1_000_000,
        ..Usage::default()
    };
    for model in [MODEL, "models/gemini-2.5-pro"] {
        assert_eq!(
            p.cost(model, &usage).map(|c| c.nano_usd),
            Some(1_250_000_000),
            "{model}"
        );
    }
}
