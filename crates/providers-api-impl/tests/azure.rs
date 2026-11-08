//! Azure OpenAI (C2.12) na serwerze fixture: adres wdrożenia z `api-version`, nagłówek `api-key`
//! zamiast `Authorization`, model Alfy → wdrożenie, lista modeli = wdrożenia konta, osadzenia;
//! pełny zestaw kontraktowy `ModelProvider`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use futures_util::StreamExt;
use providers_api_impl::{
    AccountProfile, AuthScheme, AzureOpenAiProvider, AzureOptions, CatalogEntry, ConfigError,
    ProviderProfile, build_provider,
};
use providers_contract::contract_tests::{self, Scenario};
use providers_contract::{
    CancellationToken, ChatRequest, EmbeddingRequest, Message, ModelProvider, ProviderErrorKind,
    ProviderEvent, StaticKey, StopReason,
};
use serde_json::json;
use support::openai_sse as sse;
use support::{FixtureServer, Part, Reply, WireHarness, http, http_error, slow};

const MODEL: &str = "gpt-4o";
const DEPLOYMENT: &str = "prod-4o";
const VERSION: &str = "2024-10-21";

fn options() -> AzureOptions {
    AzureOptions {
        api_version: VERSION.into(),
        deployments: BTreeMap::from([(MODEL.into(), DEPLOYMENT.into())]),
    }
}

fn entry() -> CatalogEntry {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../providers-catalog/czeka-na-wlasciciela/azure-openai.toml");
    CatalogEntry::parse_toml(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn account(server: Option<&FixtureServer>) -> AccountProfile {
    let mut a = AccountProfile::new(Arc::new(StaticKey::new("azure-key-123")));
    a.base_url = server.map(FixtureServer::url);
    a.default_model = Some(MODEL.into());
    a.azure = Some(options());
    a
}

/// Azure poprzedza odpowiedź porcją bez `choices` z wynikiem filtrów treści.
fn filter_results() -> String {
    sse::data(&json!({
        "id": "", "object": "", "created": 0, "model": "", "choices": [],
        "prompt_filter_results": [{"prompt_index": 0, "content_filter_results": {}}]
    }))
}

fn reply(s: &Scenario) -> Option<Reply> {
    Some(match s {
        Scenario::Text { chunks } => {
            let c: Vec<&str> = chunks.iter().map(String::as_str).collect();
            Reply::sse(filter_results() + &sse::chat_text(MODEL, &c, "stop"))
        }
        Scenario::ToolCall {
            id,
            name,
            arguments,
        } => Reply::sse(sse::chat_tool(MODEL, id, name, &arguments.to_string())),
        Scenario::Thinking { .. } => return None,
        Scenario::HttpError {
            status,
            retry_after_s,
        } => {
            let body = match status {
                401 => {
                    json!({"error": {"code": "401", "message": "Access denied due to invalid subscription key or wrong API endpoint."}})
                }
                429 => sse::error_body("requests", "Rate limit reached", Some("429")),
                400 | 404 => sse::error_body("invalid_request_error", "bad", None),
                _ => sse::error_body("server_error", "The server had an error", None),
            };
            http_error(*status, *retry_after_s, &body)
        }
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

fn make(server: &FixtureServer, profile: ProviderProfile) -> AzureOpenAiProvider {
    AzureOpenAiProvider::new(
        profile,
        http(server, AuthScheme::Header("api-key".into())),
        Arc::new(StaticKey::new("azure-key-123")),
        options(),
    )
    .unwrap()
}

#[tokio::test]
async fn contract_suite_azure() {
    let h = WireHarness::new("azure-openai", MODEL, Box::new(reply), Box::new(make)).await;
    contract_tests::run_all(&h).await;
}

#[tokio::test]
async fn chat_goes_to_the_deployment_with_api_key_header() {
    let server = FixtureServer::start().await;
    server.reset(
        reply(&Scenario::Text {
            chunks: vec!["Cześć".into()],
        })
        .unwrap(),
    );
    let p = build_provider(&entry(), account(Some(&server))).unwrap();
    assert_eq!(p.id().as_str(), "azure-openai");
    let events: Vec<ProviderEvent> = p
        .stream(
            ChatRequest::new(MODEL, vec![Message::user_text("hej")]),
            CancellationToken::new(),
        )
        .collect()
        .await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Cześć");
    let rec = server.requests().pop().unwrap();
    assert_eq!(rec.method, "POST");
    assert_eq!(
        rec.path,
        format!("/openai/deployments/{DEPLOYMENT}/chat/completions?api-version={VERSION}")
    );
    assert_eq!(rec.header("api-key"), Some("azure-key-123"));
    assert!(
        rec.header("authorization").is_none(),
        "klucz tylko w api-key"
    );
    assert!(rec.header("openai-organization").is_none());
    let body = rec.json();
    assert_eq!(body["model"], DEPLOYMENT);
    assert!(body.get("max_completion_tokens").is_some());
    assert_eq!(body["stream"], true);

    // Model bez wpisu wdrożenia idzie pod własną nazwą.
    server.reset(reply(&Scenario::Text { chunks: vec![] }).unwrap());
    let _: Vec<ProviderEvent> = p
        .stream(
            ChatRequest::new("o3-mini", vec![Message::user_text("hej")]),
            CancellationToken::new(),
        )
        .collect()
        .await;
    let rec = server.requests().pop().unwrap();
    assert!(
        rec.path
            .starts_with("/openai/deployments/o3-mini/chat/completions?"),
        "{}",
        rec.path
    );
}

#[tokio::test]
async fn models_are_the_account_deployments_after_a_key_check() {
    let server = FixtureServer::start().await;
    server.reset(Reply::json(
        200,
        &json!({"data": [{"id": "gpt-4o"}, {"id": "dall-e-3"}]}),
    ));
    let p = build_provider(&entry(), account(Some(&server))).unwrap();
    let models = p.list_models().await.unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, MODEL);
    assert_eq!(models[0].display_name.as_deref(), Some(DEPLOYMENT));
    let rec = server.requests().pop().unwrap();
    assert_eq!(rec.method, "GET");
    assert_eq!(rec.path, format!("/openai/models?api-version={VERSION}"));
    assert_eq!(rec.header("api-key"), Some("azure-key-123"));

    server.reset(Reply::json(
        401,
        &json!({"error": {"code": "401", "message": "Access denied due to invalid subscription key or wrong API endpoint."}}),
    ));
    let err = p.list_models().await.unwrap_err();
    assert_eq!(err.kind, ProviderErrorKind::Auth);
    assert!(!err.message.contains("azure-key-123"));
}

/// Literówka w nazwie wdrożenia przechodzi test klucza; przy rozmowie 404 `DeploymentNotFound`
/// ma dać polski komunikat z nazwami wdrożeń konta, nie surowe „HTTP 404”.
#[tokio::test]
async fn missing_deployment_gets_a_clear_message() {
    let server = FixtureServer::start().await;
    server.reset(Reply::json(
        404,
        &json!({"error": {"code": "DeploymentNotFound", "message": "The API deployment for this resource does not exist."}}),
    ));
    let p = build_provider(&entry(), account(Some(&server))).unwrap();
    let events: Vec<ProviderEvent> = p
        .stream(
            ChatRequest::new(MODEL, vec![Message::user_text("hej")]),
            CancellationToken::new(),
        )
        .collect()
        .await;
    let err = events
        .iter()
        .find_map(|e| match e {
            ProviderEvent::Error(error) => Some(error.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("brak błędu: {events:?}"));
    assert_eq!(err.kind, ProviderErrorKind::InvalidRequest);
    assert_eq!(err.status, Some(404));
    assert!(
        err.message.contains("DeploymentNotFound"),
        "{}",
        err.message
    );
    assert!(err.message.contains(DEPLOYMENT), "{}", err.message);
}

#[tokio::test]
async fn embeddings_use_the_deployment_path() {
    let server = FixtureServer::start().await;
    server.reset(Reply::json(
        200,
        &json!({"data": [{"index": 0, "embedding": [0.5, 0.25]}], "usage": {"prompt_tokens": 3}}),
    ));
    let mut a = account(Some(&server));
    a.azure = Some(AzureOptions {
        api_version: VERSION.into(),
        deployments: BTreeMap::from([("text-embedding-3-small".into(), "emb".into())]),
    });
    let p = build_provider(&entry(), a).unwrap();
    let out = p
        .embed(EmbeddingRequest {
            model: "text-embedding-3-small".into(),
            input: vec!["ala".into()],
        })
        .await
        .unwrap();
    assert_eq!(out.vectors, vec![vec![0.5, 0.25]]);
    assert_eq!(out.usage.input_tokens, 3);
    let rec = server.requests().pop().unwrap();
    assert_eq!(
        rec.path,
        format!("/openai/deployments/emb/embeddings?api-version={VERSION}")
    );
    assert_eq!(rec.json(), json!({"input": ["ala"]}));
}

#[test]
fn azure_needs_the_resource_address_from_the_account() {
    assert!(matches!(
        build_provider(&entry(), account(None)),
        Err(ConfigError::MissingBaseUrl(id)) if id == "azure-openai"
    ));
}
