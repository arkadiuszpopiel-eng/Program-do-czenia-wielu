//! Router jako `ModelProvider`: zdrowie, możliwości (`dostawca:model`), koszt, przypięcie, osadzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use providers_contract::{HealthState, ModelProvider, ProviderErrorKind, ProviderEvent, Usage};
use router_contract::{RouteKind, TaskClass};
use router_impl::{RoutedProvider, RouterCore};
use support::*;

#[tokio::test]
async fn routed_provider_surface() {
    let core = Arc::new(RouterCore::default());
    let routed = RoutedProvider::new(core.clone(), TaskClass::Conversation);
    assert_eq!(routed.health().state, HealthState::Unconfigured);
    let events = collect(&routed, req("x")).await;
    assert!(
        matches!(&events[..], [ProviderEvent::Error(e)] if e.kind == ProviderErrorKind::Unsupported)
    );
    core.register(Arc::new(fake("alpha")), RouteKind::Api);
    let embedder = providers_contract::ModelCapabilities {
        kinds: vec![providers_contract::ModelKind::Embeddings],
        ..providers_contract::ModelCapabilities::default()
    };
    core.register(
        Arc::new(fake("local").with_model("emb", embedder)),
        RouteKind::Local,
    );
    assert_eq!(routed.id().as_str(), "router");
    assert_eq!(routed.class(), TaskClass::Conversation);
    assert_eq!(routed.health().state, HealthState::Healthy);
    let caps = routed.capabilities();
    assert!(caps.models.contains_key("alpha:fake-model"));
    assert_eq!(caps.default_model.as_deref(), Some("auto"));
    assert_eq!(routed.list_models().await.unwrap().len(), 3);
    let usage = Usage {
        input_tokens: 1_000_000,
        ..Usage::default()
    };
    assert_eq!(
        routed.cost("alpha:fake-model", &usage).map(|c| c.nano_usd),
        Some(4_000_000_000)
    );
    assert_eq!(routed.cost("fake-model", &usage), None);
    assert!(routed.estimate_cost(&req("ile")).is_some());
    // Przypięcie `dostawca:model`.
    let mut pinned = req("x");
    pinned.model = "local:fake-model".into();
    let events = collect(&routed, pinned).await;
    assert!(
        matches!(&events[0], ProviderEvent::Started { model, .. } if model == "local:fake-model")
    );
    // Osadzenia z fallbackiem.
    let emb = RoutedProvider::new(core.clone(), TaskClass::Embeddings)
        .background(true)
        .max_latency_ms(None);
    let out = emb
        .embed(providers_contract::EmbeddingRequest {
            model: "local:emb".into(),
            input: vec!["a".into()],
        })
        .await
        .unwrap();
    assert_eq!(out.vectors.len(), 1);
}
