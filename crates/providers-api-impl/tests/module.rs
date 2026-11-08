//! Moduł w rejestrze: manifest, cykl życia, zdarzenia `provider.*` na magistrali, symulacja
//! fallbacku Routera (ACC-F1-providers-api-02: 5xx → inny cel ≤ 2 s, ta sama historia).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;
use std::time::Duration;

use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use futures_util::StreamExt;
use providers_api_impl::{
    AnthropicOptions, AnthropicProvider, AuthScheme, MODULE_TOML, ProvidersApiModule,
};
use providers_contract::{
    CancellationToken, ChatRequest, Message, ModelProvider, ProviderEvent, StaticKey, StopReason,
    events,
};
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use support::{FixtureServer, Reply, anthropic_sse, http, http_error, profile};
use tokio::time::Instant;

const MODEL: &str = "claude-opus-5-5";

fn anthropic(server: &FixtureServer) -> Arc<dyn ModelProvider> {
    let mut p = profile("anthropic", MODEL);
    p.privacy = providers_contract::ProviderPrivacy::new("eu", "EU");
    Arc::new(
        AnthropicProvider::new(
            p,
            http(server, AuthScheme::XApiKey),
            Arc::new(StaticKey::new("k")),
            AnthropicOptions::native(),
        )
        .unwrap(),
    )
}

#[test]
fn manifest_is_valid() {
    let m = ProvidersApiModule::new().unwrap();
    let manifest = m.manifest();
    assert_eq!(manifest.id.as_str(), "providers-api");
    assert_eq!(manifest.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest.lifecycle, Lifecycle::Lazy);
    assert_eq!(manifest.provides[0].to_string(), "providers-contract@1");
    assert!(MODULE_TOML.contains("net.egress"));
}

#[tokio::test]
async fn lifecycle_events_and_health() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(anthropic_sse::response(
        MODEL,
        &[anthropic_sse::Block::Text(vec!["ok".into()])],
        "refusal",
        Some(serde_json::json!({"category": "cyber"})),
    )));
    let mut module = ProvidersApiModule::new().unwrap();
    assert_eq!(module.health(), HealthStatus::NotStarted);
    assert_eq!(module.stop().await, Err(ModuleError::NotStarted));
    module.register(anthropic(&server));
    module.register(anthropic(&server)); // ten sam id — zastępuje
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx.clone()).await.unwrap();
    assert_eq!(module.start(ctx).await, Err(ModuleError::AlreadyStarted));
    assert_eq!(module.health(), HealthStatus::Healthy);
    let providers = module.providers();
    assert_eq!(providers.len(), 1);

    let mut req = ChatRequest::new(MODEL, vec![Message::user_text("tajna treść")]);
    req.meta.session = Some("s-1".into());
    let events_seen: Vec<_> = providers[0]
        .stream(req, CancellationToken::new())
        .collect()
        .await;
    assert!(matches!(
        events_seen.last(),
        Some(ProviderEvent::Stop {
            reason: StopReason::Refusal,
            ..
        })
    ));
    let calls = bus.recorded_of_kind(&EventKind::ModelCall);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].payload["name"], events::CALL_STARTED);
    let finished = &calls[1];
    assert_eq!(finished.payload["name"], events::CALL_FINISHED);
    assert_eq!(finished.payload["usage"]["output_tokens"], 42);
    assert_eq!(finished.session.as_ref().map(|s| s.as_str()), Some("s-1"));
    let cost = finished.cost.clone().unwrap();
    assert_eq!(cost.input_tokens, 25 + 7 + 100);
    assert!(cost.micro_usd > 0, "koszt z cennika konfiguracji");
    assert_eq!(
        bus.recorded_of_kind(&EventKind::Custom(events::REFUSAL.into()))
            .len(),
        1
    );
    let all = format!("{:?}", bus.recorded());
    assert!(!all.contains("tajna treść"), "zdarzenia bez treści rozmowy");

    server.reset(http_error(
        401,
        None,
        &anthropic_sse::error_body("authentication_error", "bad key"),
    ));
    let _: Vec<_> = providers[0]
        .stream(
            ChatRequest::new(MODEL, vec![Message::user_text("x")]),
            CancellationToken::new(),
        )
        .collect()
        .await;
    assert_eq!(
        bus.recorded_of_kind(&EventKind::Custom(events::ERROR.into()))
            .len(),
        1
    );
    assert!(matches!(module.health(), HealthStatus::Degraded(_)));
    module.stop().await.unwrap();
    assert_eq!(module.health(), HealthStatus::NotStarted);
}

#[tokio::test]
async fn router_style_fallback_within_two_seconds() {
    let server = FixtureServer::start().await;
    server.reset(http_error(
        503,
        None,
        &anthropic_sse::error_body("overloaded_error", "Overloaded"),
    ));
    let primary = anthropic(&server);
    let backup = FakeProvider::new("local")
        .with_default_script(Script::text(FAKE_MODEL, &["Odpowiedź zapasowa"]));
    let history = vec![
        Message::user_text("Pytanie"),
        Message::assistant_text("…"),
        Message::user_text("I co dalej?"),
    ];

    let start = Instant::now();
    let first: Vec<_> = primary
        .stream(
            ChatRequest::new(MODEL, history.clone()),
            CancellationToken::new(),
        )
        .collect()
        .await;
    let err = match first.last() {
        Some(ProviderEvent::Error(e)) => e.clone(),
        other => panic!("{other:?}"),
    };
    assert!(err.should_fallback() && !err.after_output);
    let second: Vec<_> = backup
        .stream(
            ChatRequest::new(FAKE_MODEL, history.clone()),
            CancellationToken::new(),
        )
        .collect()
        .await;
    assert_eq!(
        second.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "fallback po {:?}",
        start.elapsed()
    );
    assert_eq!(
        backup.requests()[0].messages,
        history,
        "0 utraconych wiadomości"
    );
}
