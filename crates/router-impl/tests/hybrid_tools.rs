//! Historia narzędzi i brak powtórnego wykonania przy zmianie lokalnie/API.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use providers_contract::{
    ContentBlock, Message, ProviderError, ProviderErrorKind, ProviderEvent, Role, ToolResult,
    ToolResultPart, ToolSpec, ToolUse,
};
use providers_fake::{FAKE_MODEL, Script};
use router_contract::{RouteKind, TaskClass};
use router_impl::{RoutedProvider, RouterCore};
use support::{collect, fake, req, text};

fn request() -> providers_contract::ChatRequest {
    let mut r = req("Pobierz publiczny opis.");
    r.model = format!("alpha:{FAKE_MODEL}");
    r.system = Some("Podsumuj wyłącznie zwrócone źródło, zachowaj jego adres.".into());
    r.tools.push(ToolSpec {
        name: "net_fetch".into(),
        description: "Odczyt publicznego źródła HTTPS".into(),
        input_schema: serde_json::json!({"type":"object","properties":{"url":{"type":"string"}},"required":["url"],"additionalProperties":false}),
        strict: true,
    });
    r.messages.push(Message::new(
        Role::Assistant,
        vec![ContentBlock::ToolUse(ToolUse {
            id: "fetch-1".into(),
            name: "net_fetch".into(),
            input: serde_json::json!({"url":"https://example.com/"}),
        })],
    ));
    r.messages.push(Message::new(
        Role::User,
        vec![
            ContentBlock::ToolResult(ToolResult {
                tool_use_id: "fetch-1".into(),
                content: vec![ToolResultPart::Text {
                    text: "Źródło https://example.com/ — zażółć gęślą jaźń.".into(),
                }],
                is_error: false,
            }),
            ContentBlock::text("Podsumuj wynik bez ponownego pobierania."),
        ],
    ));
    r
}

#[tokio::test(start_paused = true)]
async fn failover_preserves_tool_ids_results_and_schema_in_both_directions() {
    for (from, to) in [
        (RouteKind::Local, RouteKind::Api),
        (RouteKind::Api, RouteKind::Local),
    ] {
        let primary = fake("alpha");
        let backup = fake("beta");
        primary.push_script(Script::http_error(503, None));
        let core = Arc::new(RouterCore::default());
        core.register(Arc::new(primary.clone()), from);
        core.register(Arc::new(backup.clone()), to);
        let routed = RoutedProvider::new(core, TaskClass::Conversation);
        let original = request();
        let events = collect(&routed, original.clone()).await;
        assert_eq!(text(&events), "beta odpowiada");
        assert_eq!(primary.requests().len(), 1);
        let sent = backup.requests();
        assert_eq!(sent.len(), 1);
        let mut expected = original;
        expected.model = FAKE_MODEL.into();
        assert_eq!(
            sent[0], expected,
            "całe żądanie bez zmian poza nazwą modelu"
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, ProviderEvent::Started { .. }))
                .count(),
            1
        );
    }
}

#[tokio::test(start_paused = true)]
async fn standalone_local_and_api_preserve_tool_history() {
    for kind in [RouteKind::Local, RouteKind::Api] {
        let primary = fake("alpha");
        let core = Arc::new(RouterCore::default());
        core.register(Arc::new(primary.clone()), kind);
        let routed = RoutedProvider::new(core, TaskClass::Conversation);
        let original = request();
        assert_eq!(
            text(&collect(&routed, original.clone()).await),
            "alpha odpowiada"
        );
        let mut expected = original;
        expected.model = FAKE_MODEL.into();
        assert_eq!(primary.requests(), vec![expected]);
    }
}

#[tokio::test(start_paused = true)]
async fn error_after_tool_start_never_replays_on_other_route() {
    for (from, to) in [
        (RouteKind::Local, RouteKind::Api),
        (RouteKind::Api, RouteKind::Local),
    ] {
        let primary = fake("alpha");
        let backup = fake("beta");
        let mut script = Script::tool_call(
            FAKE_MODEL,
            "fetch-2",
            "net_fetch",
            &serde_json::json!({"url":"https://example.com/"}),
        );
        script.steps.truncate(2); // Started i początek narzędzia przed awarią połączenia.
        script
            .steps
            .push(providers_fake::Step::Emit(ProviderEvent::Error(
                ProviderError::new(ProviderErrorKind::Network, "przerwane połączenie"),
            )));
        primary.push_script(script);
        let core = Arc::new(RouterCore::default());
        core.register(Arc::new(primary), from);
        core.register(Arc::new(backup.clone()), to);
        let events = collect(
            &RoutedProvider::new(core, TaskClass::Conversation),
            request(),
        )
        .await;
        assert!(matches!(events.last(), Some(ProviderEvent::Error(e)) if e.after_output));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, ProviderEvent::ToolCallStart { id, .. } if id=="fetch-2"))
                .count(),
            1
        );
        assert!(
            backup.requests().is_empty(),
            "rozpoczęte narzędzie nie jest odtwarzane"
        );
    }
}
