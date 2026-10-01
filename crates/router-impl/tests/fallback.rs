//! ACC-F1-router-02 / F1-04: sztuczny 5xx/timeout → fallback ≤ 2 s bez utraty wiadomości
//! (50 prób na atrapie, czas wirtualny + pomiar w czasie rzeczywistym); błąd po częściowym
//! wyjściu nie jest maskowany; anulowanie; pochodzenie bloków myślenia przez Router.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ContentBlock, Message, ModelProvider, ProviderError, ProviderErrorKind,
    ProviderEvent, StopReason, TurnAccumulator,
};
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use router_contract::{RouteKind, TaskClass};
use router_impl::{RoutedProvider, RouterCore};
use support::{collect, fake, req, text};
use tokio::time::Instant;

fn setup() -> (Arc<RouterCore>, FakeProvider, FakeProvider, RoutedProvider) {
    let primary = fake("alpha");
    let backup = fake("beta");
    let core = Arc::new(RouterCore::default());
    core.register(Arc::new(primary.clone()), RouteKind::Api);
    core.register(Arc::new(backup.clone()), RouteKind::Api);
    let routed = RoutedProvider::new(core.clone(), TaskClass::Conversation);
    (core, primary, backup, routed)
}

/// Czas od startu żądania do pierwszej treści i pełny strumień.
async fn first_content(
    p: &dyn ModelProvider,
    r: providers_contract::ChatRequest,
) -> (Duration, Vec<ProviderEvent>) {
    let t0 = Instant::now();
    let mut s = p.stream(r, CancellationToken::new());
    let mut at = None;
    let mut out = Vec::new();
    while let Some(ev) = s.next().await {
        if at.is_none() && ev.is_content() {
            at = Some(t0.elapsed());
        }
        out.push(ev);
    }
    (at.unwrap_or(Duration::MAX), out)
}

fn assert_switched(events: &[ProviderEvent], trial: usize) {
    assert_eq!(text(events), "beta odpowiada", "próba {trial}: {events:?}");
    let starts: Vec<&ProviderEvent> = events
        .iter()
        .filter(|e| matches!(e, ProviderEvent::Started { .. }))
        .collect();
    assert_eq!(starts.len(), 1, "jedno `Started` (od celu zapasowego)");
    assert!(
        matches!(starts[0], ProviderEvent::Started { model, .. } if model == "beta:fake-model")
    );
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
}

#[tokio::test(start_paused = true)]
async fn fifty_trials_5xx_and_timeout_switch_within_2s_without_loss() {
    for trial in 0..50 {
        let (core, primary, backup, routed) = setup();
        if trial % 2 == 0 {
            primary.push_script(Script::http_error(503, None));
        } else {
            // Milczący dostawca: termin pierwszego zdarzenia klasy (1,5 s) → przełączenie.
            primary.push_script(Script::stall());
        }
        let original = req(&format!("Wiadomość nr {trial} — zażółć gęślą jaźń"));
        let (elapsed, events) = first_content(&routed, original.clone()).await;
        assert!(
            elapsed <= Duration::from_secs(2),
            "próba {trial}: {elapsed:?}"
        );
        assert_switched(&events, trial);
        // 0 utraconych wiadomości: zapasowy cel dostał identyczną historię.
        let got = backup.requests();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].messages, original.messages, "próba {trial}");
        assert_eq!(got[0].model, FAKE_MODEL);
        assert_eq!(primary.calls().len(), 1);
        drop(core);
    }
}

#[tokio::test]
async fn real_time_measurement_5xx_and_timeout() {
    let (_core, primary, _backup, routed) = setup();
    primary.push_script(Script::http_error(500, None));
    let (e5xx, events) = first_content(&routed, req("5xx")).await;
    assert_switched(&events, 0);
    let (_core, primary, _backup, routed) = setup();
    primary.push_script(Script::stall());
    let (etimeout, events) = first_content(&routed, req("timeout")).await;
    assert_switched(&events, 1);
    eprintln!("pomiar fallbacku (czas rzeczywisty): 5xx {e5xx:?}, timeout {etimeout:?}");
    assert!(
        e5xx <= support::budget(Duration::from_millis(200)),
        "{e5xx:?}"
    );
    assert!(
        etimeout <= support::budget(Duration::from_secs(2)),
        "{etimeout:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn error_after_partial_output_is_not_masked() {
    let (_core, primary, backup, routed) = setup();
    let mut script = Script::text(FAKE_MODEL, &["Częściowa ", "odpowiedź"]);
    script.steps.truncate(3); // Started + 2 fragmenty
    script
        .steps
        .push(providers_fake::Step::Emit(ProviderEvent::Error(
            ProviderError::new(ProviderErrorKind::Network, "urwane połączenie"),
        )));
    primary.push_script(script);
    let events = collect(&routed, req("x")).await;
    assert_eq!(text(&events), "Częściowa odpowiedź");
    match events.last() {
        Some(ProviderEvent::Error(e)) => assert!(e.after_output && e.should_fallback()),
        other => panic!("{other:?}"),
    }
    assert!(backup.requests().is_empty(), "bez cichego przełączenia");
    let mut acc = TurnAccumulator::new(routed.id().clone());
    events.iter().for_each(|e| acc.push(e));
    let turn = acc.finish();
    assert!(!turn.is_complete());
    assert_eq!(turn.message.visible_text(), "Częściowa odpowiedź");
}

#[tokio::test(start_paused = true)]
async fn non_fallback_errors_and_last_target_errors_are_returned() {
    let (_core, primary, backup, routed) = setup();
    primary.push_script(Script::http_error(400, None));
    let events = collect(&routed, req("x")).await;
    assert!(
        matches!(events.last(), Some(ProviderEvent::Error(e)) if e.kind == ProviderErrorKind::InvalidRequest)
    );
    assert!(backup.requests().is_empty());
    // Oba cele padają → błąd ostatniego.
    let (_core, primary, backup, routed) = setup();
    primary.push_script(Script::http_error(503, None));
    backup.push_script(Script::http_error(502, None));
    let events = collect(&routed, req("x")).await;
    assert!(
        matches!(events.last(), Some(ProviderEvent::Error(e)) if e.kind == ProviderErrorKind::Server { status: 502 })
    );
    // Nieprawidłowe żądanie odrzucane lokalnie, bez decyzji i bez ruchu.
    let (_core, primary, _backup, routed) = setup();
    let empty = providers_contract::ChatRequest::new("auto", vec![]);
    let events = collect(&routed, empty).await;
    assert!(
        matches!(&events[..], [ProviderEvent::Error(e)] if e.kind == ProviderErrorKind::InvalidRequest)
    );
    assert!(primary.calls().is_empty());
}

#[tokio::test]
async fn cancel_through_router_is_fast() {
    let (_core, primary, _backup, routed) = setup();
    let chunks: Vec<String> = (0..50).map(|i| format!("s{i} ")).collect();
    primary.push_script(Script::chunks(
        FAKE_MODEL,
        &chunks,
        Duration::from_millis(40),
    ));
    let cancel = CancellationToken::new();
    let mut s = routed.stream(req("x"), cancel.clone());
    while let Some(ev) = s.next().await {
        if ev.is_content() {
            break;
        }
    }
    let t = Instant::now();
    cancel.cancel();
    let mut last = None;
    while let Some(ev) = s.next().await {
        last = Some(ev);
    }
    let took = t.elapsed();
    eprintln!("anulowanie przez Router: {took:?} (budżet 100 ms)");
    assert!(
        took <= support::budget(Duration::from_millis(100)),
        "{took:?}"
    );
    assert_eq!(last, Some(ProviderEvent::stop(StopReason::Cancelled)));
    let pre = CancellationToken::new();
    pre.cancel();
    let events: Vec<_> = routed.stream(req("y"), pre).collect().await;
    assert_eq!(events, [ProviderEvent::stop(StopReason::Cancelled)]);
}

#[tokio::test(start_paused = true)]
async fn thinking_origin_survives_router_round_trip() {
    let (_core, primary, _backup, routed) = setup();
    primary.push_script(Script::thinking(FAKE_MODEL, "myślę", "sig-abc", "Wynik."));
    let first = req("pytanie");
    let events = collect(&routed, first.clone()).await;
    let mut acc = TurnAccumulator::new(routed.id().clone());
    events.iter().for_each(|e| acc.push(e));
    let turn = acc.finish();
    let origin = turn.message.content.iter().find_map(|b| match b {
        ContentBlock::Thinking(t) => Some(t.provider_origin.clone()),
        _ => None,
    });
    let origin = origin.unwrap();
    assert_eq!(origin.provider.as_str(), "router");
    assert_eq!(origin.model, "alpha:fake-model");
    let mut next = first;
    next.messages.push(turn.message.clone());
    next.messages.push(Message::user_text("dalej"));
    collect(&routed, next).await;
    let wire = primary.requests().pop().unwrap();
    let back = wire.messages[1].content.iter().find_map(|b| match b {
        ContentBlock::Thinking(t) => Some(t.provider_origin.clone()),
        _ => None,
    });
    let back = back.unwrap();
    assert_eq!(
        back.provider.as_str(),
        "alpha",
        "adapter dostaje własne pochodzenie"
    );
    assert_eq!(back.model, FAKE_MODEL);
}
