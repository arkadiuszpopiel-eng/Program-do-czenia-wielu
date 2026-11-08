//! Testy atrapy: wirtualny zegar, wstrzykiwanie błędów, record/replay, weryfikacja żądań, zdrowie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, EmbeddingRequest, HealthState, Message, ModelProvider,
    ProviderError, ProviderErrorKind, ProviderEvent, StopReason, TimeoutPhase, TurnAccumulator,
};
use providers_fake::{
    FAKE_MODEL, FakeProvider, FakeTimeouts, Recorder, Script, Step, StreamSpeed, from_ndjson,
};
use tokio::time::Instant;

fn req() -> ChatRequest {
    ChatRequest::new(FAKE_MODEL, vec![Message::user_text("hej")])
}

async fn run(p: &impl ModelProvider) -> Vec<ProviderEvent> {
    p.stream(req(), CancellationToken::new()).collect().await
}

#[tokio::test(start_paused = true)]
async fn virtual_clock_gives_exact_ttft_and_speed() {
    let fake = FakeProvider::new("f");
    let speed = StreamSpeed {
        ttft: Duration::from_millis(700),
        tokens_per_sec: 20.0,
    };
    fake.push_script(Script::streamed(FAKE_MODEL, "jeden dwa trzy cztery", speed));
    let start = Instant::now();
    let mut stream = fake.stream(req(), CancellationToken::new());
    let mut first = None;
    while let Some(ev) = stream.next().await {
        if first.is_none() && ev.is_content() {
            first = Some(start.elapsed());
        }
    }
    assert_eq!(first, Some(Duration::from_millis(700)));
    assert_eq!(start.elapsed(), Duration::from_millis(700 + 3 * 50));
    assert_eq!(fake.health().last_ttft_ms, Some(700));
}

#[tokio::test(start_paused = true)]
async fn injected_errors_then_recovery_update_health() {
    let fake = FakeProvider::new("f").with_default_script(Script::text(FAKE_MODEL, &["ok"]));
    for _ in 0..3 {
        fake.fail_next(ProviderError::new(
            ProviderErrorKind::Server { status: 503 },
            "atrapa",
        ));
    }
    assert_eq!(fake.pending_scripts(), 3);
    for i in 1..=3u32 {
        let events = run(&fake).await;
        assert!(matches!(events.last(), Some(ProviderEvent::Error(_))));
        assert_eq!(fake.health().consecutive_failures, i);
    }
    assert_eq!(fake.health().state, HealthState::Unavailable);
    let events = run(&fake).await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
    assert_eq!(fake.health().state, HealthState::Healthy);
    assert_eq!(fake.requests().len(), 4);
}

#[tokio::test(start_paused = true)]
async fn idle_timeout_after_output_is_marked() {
    let fake = FakeProvider::new("f").with_timeouts(FakeTimeouts {
        first_token: None,
        idle: Some(Duration::from_millis(100)),
    });
    let mut script = Script::text(FAKE_MODEL, &["a"]);
    script.steps.insert(2, Step::Stall);
    fake.push_script(script);
    let events = run(&fake).await;
    match events.last() {
        Some(ProviderEvent::Error(e)) => {
            assert_eq!(
                e.kind,
                ProviderErrorKind::Timeout {
                    phase: TimeoutPhase::Idle
                }
            );
            assert!(
                e.after_output,
                "błąd po treści: Router odrzuca część i powtarza"
            );
            assert!(!e.is_retryable());
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test(start_paused = true)]
async fn missing_script_and_truncated_script_are_protocol_errors() {
    let fake = FakeProvider::new("f");
    let events = run(&fake).await;
    assert!(
        matches!(events.as_slice(), [ProviderEvent::Error(e)] if e.kind == ProviderErrorKind::Protocol)
    );
    fake.push_script(Script::new(vec![Step::Emit(ProviderEvent::Started {
        model: FAKE_MODEL.into(),
        response_id: None,
    })]));
    let events = run(&fake).await;
    assert!(
        matches!(events.last(), Some(ProviderEvent::Error(e)) if e.kind == ProviderErrorKind::Protocol)
    );
}

#[tokio::test(start_paused = true)]
async fn record_then_replay_is_identical() {
    let live = FakeProvider::new("live");
    live.push_script(Script::chunks(
        FAKE_MODEL,
        &["Raz ".into(), "dwa".into()],
        Duration::from_millis(25),
    ));
    live.push_script(Script::tool_call(
        FAKE_MODEL,
        "t1",
        "clock",
        &serde_json::json!({"tz": "PL"}),
    ));
    let recorder = Recorder::new(live);
    let first = run(&recorder).await;
    let second = run(&recorder).await;
    let ndjson = recorder.cassette_ndjson();
    assert_eq!(from_ndjson(&ndjson).unwrap().len(), 2);
    assert_eq!(recorder.entries()[0].events[2].at_ms, 25);

    let replay = FakeProvider::from_cassette("replay", &ndjson).unwrap();
    let start = Instant::now();
    assert_eq!(run(&replay).await, first);
    assert_eq!(
        start.elapsed(),
        Duration::from_millis(25),
        "odstępy odtworzone wirtualnie"
    );
    assert_eq!(run(&replay).await, second);
    assert!(FakeProvider::from_cassette("x", "nie-json").is_err());
}

#[tokio::test(start_paused = true)]
async fn requests_are_recorded_after_projection() {
    let fake = FakeProvider::new("f").with_default_script(Script::text(FAKE_MODEL, &["ok"]));
    let mut r = req();
    r.messages
        .push(Message::assistant_text("Długa odpowiedź").with_interruption("Długa", true));
    r.messages.push(Message::user_text("dalej"));
    let events = run_req(&fake, r.clone()).await;
    let mut acc = TurnAccumulator::new(fake.id().clone());
    events.iter().for_each(|e| acc.push(e));
    assert_eq!(acc.finish().message.visible_text(), "ok");
    let wire = fake.requests().pop().unwrap();
    assert_eq!(wire.messages.len(), 4, "notka po przerwanej turze");
    assert_eq!(wire.messages[1], r.messages[1]);
    assert_eq!(fake.calls().len(), 1);
}

async fn run_req(p: &impl ModelProvider, r: ChatRequest) -> Vec<ProviderEvent> {
    p.stream(r, CancellationToken::new()).collect().await
}

#[tokio::test]
async fn embeddings_and_models() {
    let fake = FakeProvider::new("f");
    let out = fake
        .embed(EmbeddingRequest {
            model: "e".into(),
            input: vec!["a".into(), "bb".into()],
        })
        .await
        .unwrap();
    assert_eq!(out.vectors.len(), 2);
    assert_eq!(out.vectors[0].len(), 8);
    let models = fake.list_models().await.unwrap();
    assert_eq!(models[0].id, FAKE_MODEL);
    assert!(fake.capabilities().models.contains_key(FAKE_MODEL));
    assert_eq!(
        fake.estimate_cost(&req()),
        None,
        "brak cennika → brak oszacowania"
    );
}
