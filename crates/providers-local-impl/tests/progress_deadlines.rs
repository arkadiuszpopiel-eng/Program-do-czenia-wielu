//! Metadane i pingi nie są postępem generacji; istniejące budżety zostają niezmienione.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;

use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, Message, ModelProvider, ProviderErrorKind, ProviderEvent,
    StopReason, TimeoutPhase,
};
use serde_json::{Value, json};
use std::time::Duration;

async fn run(scenario: Value) -> Vec<ProviderEvent> {
    let env = support::Env::new();
    let mut config = env.config();
    config.timeouts.first_token = Duration::from_millis(900);
    config.timeouts.idle = Duration::from_millis(300);
    let provider = env.provider_with(
        config,
        env.launcher("ok", Some(&scenario.to_string())),
        None,
        None,
    );
    let request = ChatRequest::new(support::MODEL, vec![Message::user_text("Działaj")]);
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        provider.stream(request, CancellationToken::new()).collect(),
    )
    .await;
    provider.sidecar().stop("koniec testu postępu").await;
    result.expect("same pingi lub metadane nie podtrzymują strumienia bez końca")
}

fn assert_timeout(events: &[ProviderEvent], phase: TimeoutPhase, after_output: bool) {
    assert!(
        matches!(events.last(), Some(ProviderEvent::Error(error))
        if error.kind==ProviderErrorKind::Timeout { phase } && error.after_output==after_output),
        "{events:?}"
    );
    assert_eq!(events.iter().filter(|e| e.is_terminal()).count(), 1);
}

#[tokio::test]
async fn comments_do_not_extend_the_first_content_deadline() {
    let events = run(json!({"kind":"heartbeat_stall"})).await;
    assert_timeout(&events, TimeoutPhase::FirstToken, false);
}

#[tokio::test]
async fn metadata_and_empty_deltas_do_not_extend_the_first_content_deadline() {
    let events = run(json!({"kind":"heartbeat_stall","metadata":true})).await;
    assert_timeout(&events, TimeoutPhase::FirstToken, false);
}

#[tokio::test]
async fn comments_after_content_do_not_extend_the_idle_deadline() {
    let events = run(json!({"kind":"heartbeat_stall","first_text":"Wstęp"})).await;
    assert_timeout(&events, TimeoutPhase::Idle, true);
}

#[tokio::test]
async fn initial_metadata_does_not_shorten_first_content_budget_to_idle() {
    let events = run(json!({"kind":"delayed_first"})).await;
    assert!(matches!(
        events.last(),
        Some(ProviderEvent::Stop {
            reason: StopReason::EndTurn,
            ..
        })
    ));
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Odpowiedź");
}

#[tokio::test]
async fn stop_during_metadata_stream_remains_cancellation_without_retry() {
    let env = support::Env::new();
    let provider = env.provider("ok", Some(r#"{"kind":"heartbeat_stall","metadata":true}"#));
    let cancel = CancellationToken::new();
    let request = ChatRequest::new(support::MODEL, vec![Message::user_text("Działaj")]);
    let stream = provider.stream(request, cancel.clone());
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        // ChatDecoder publikuje Usage dopiero na końcu. Sygnałem gotowości fixture
        // jest odebrane żądanie HTTP, bez oczekiwania na generację lub terminalne zdarzenie.
        let (events, ()) = tokio::join!(stream.collect::<Vec<_>>(), async {
            while env.requests().is_empty() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            cancel.cancel();
        });
        events
    })
    .await;
    provider.sidecar().stop("koniec testu Stop").await;
    let events = result.unwrap();
    assert!(matches!(
        events.last(),
        Some(ProviderEvent::Stop {
            reason: StopReason::Cancelled,
            ..
        })
    ));
    assert!(!events.iter().any(|e| matches!(e, ProviderEvent::Error(_))));
    assert_eq!(env.requests().len(), 1);
}
