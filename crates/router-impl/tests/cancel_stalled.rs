//! Stop musi kończyć trasę również wtedy, gdy adapter nie obsługuje tokenu anulowania.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, ModelInfo, ModelProvider,
    ProviderCapabilities, ProviderError, ProviderEvent, ProviderHealth, ProviderId, ProviderStream,
    StopReason, Usage,
};
use providers_fake::{FAKE_MODEL, FakeProvider};
use router_contract::{BreakerState, Outcome, RouteKind, Router, TaskClass};
use router_impl::{RoutedProvider, RouterCore};
use tokio::sync::Notify;

struct Dropped(Arc<AtomicBool>);

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

struct StalledProvider {
    fake: FakeProvider,
    prefix: Vec<ProviderEvent>,
    waiting: Arc<Notify>,
    dropped: Arc<AtomicBool>,
}

#[async_trait]
impl ModelProvider for StalledProvider {
    fn id(&self) -> &ProviderId {
        self.fake.id()
    }
    fn capabilities(&self) -> ProviderCapabilities {
        self.fake.capabilities()
    }
    fn health(&self) -> ProviderHealth {
        self.fake.health()
    }
    fn estimate_cost(&self, r: &ChatRequest) -> Option<CostEstimate> {
        self.fake.estimate_cost(r)
    }
    fn cost(&self, m: &str, u: &Usage) -> Option<Cost> {
        self.fake.cost(m, u)
    }
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.fake.list_models().await
    }
    fn stream(&self, _: ChatRequest, _: CancellationToken) -> ProviderStream {
        // Celowo ignoruje token; sygnał potwierdza rzeczywiste wejście w oczekiwanie.
        let mut prefix = self.prefix.clone().into_iter();
        let waiting = self.waiting.clone();
        let guard = Dropped(self.dropped.clone());
        Box::pin(futures_util::stream::poll_fn(move |_| {
            let _keep_alive = &guard;
            if let Some(event) = prefix.next() {
                Poll::Ready(Some(event))
            } else {
                waiting.notify_one();
                Poll::Pending
            }
        }))
    }
}

async fn cancel_case(
    kind: RouteKind,
    prefix: Vec<ProviderEvent>,
    with_backup: bool,
    half_open: bool,
) {
    let waiting = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let core = Arc::new(RouterCore::default());
    core.register(
        Arc::new(StalledProvider {
            fake: support::fake("alpha"),
            prefix: prefix.clone(),
            waiting: waiting.clone(),
            dropped: dropped.clone(),
        }),
        kind,
    );
    let backup = support::fake("beta");
    if with_backup {
        core.register(Arc::new(backup.clone()), RouteKind::Api);
    }
    let candidate = support::cand("alpha");
    if half_open {
        for _ in 0..core.policy().breaker.failures {
            core.report(
                &candidate,
                Outcome::Failed {
                    kind: providers_contract::ProviderErrorKind::Network,
                },
            );
        }
        tokio::time::advance(Duration::from_millis(core.policy().breaker.cooldown_ms)).await;
    }
    let routed = RoutedProvider::new(core.clone(), TaskClass::Conversation);
    let cancel = CancellationToken::new();
    let mut request = support::req("Zatrzymaj zadanie bez ponowienia.");
    request.model = format!("alpha:{FAKE_MODEL}");
    let stream = routed.stream(request, cancel.clone());
    waiting.notified().await;
    if half_open {
        assert_eq!(
            core.breaker_state(&candidate.provider),
            BreakerState::HalfOpen {
                trial_in_flight: true
            }
        );
    }
    cancel.cancel();
    let events: Vec<_> = tokio::time::timeout(Duration::from_millis(100), stream.collect())
        .await
        .expect("Router nadal czeka na zawieszony adapter po Stop");
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::Cancelled))
    );
    assert!(backup.requests().is_empty(), "Stop nie uruchamia zapasu");
    assert!(dropped.load(Ordering::SeqCst), "strumień celu zwolniony");
    assert_eq!(support::text(&events), support::text(&prefix));
    for event in prefix.iter().filter(|e| e.is_content()) {
        assert!(
            events.contains(event),
            "zachowana rozpoczęta treść: {event:?}"
        );
    }
    assert_eq!(events.iter().filter(|e| e.is_terminal()).count(), 1);
    assert_eq!(
        core.breaker_state(&candidate.provider),
        if half_open {
            BreakerState::HalfOpen {
                trial_in_flight: false,
            }
        } else {
            BreakerState::Closed
        }
    );
}

#[tokio::test(start_paused = true)]
async fn cancel_silent_adapter_before_first_event() {
    for kind in [RouteKind::Local, RouteKind::Api] {
        for backup in [false, true] {
            cancel_case(kind, vec![], backup, false).await;
        }
    }
}

#[tokio::test(start_paused = true)]
async fn cancel_silent_adapter_after_started() {
    for kind in [RouteKind::Local, RouteKind::Api] {
        cancel_case(
            kind,
            vec![ProviderEvent::Started {
                model: FAKE_MODEL.into(),
                response_id: None,
            }],
            true,
            false,
        )
        .await;
    }
}

#[tokio::test(start_paused = true)]
async fn cancel_silent_adapter_preserves_partial_answer() {
    for kind in [RouteKind::Local, RouteKind::Api] {
        cancel_case(
            kind,
            vec![ProviderEvent::TextDelta {
                index: 0,
                text: "Zażółć gęślą jaźń — wynik częściowy.".into(),
            }],
            true,
            false,
        )
        .await;
    }
}

#[tokio::test(start_paused = true)]
async fn cancellation_releases_half_open_trial_without_penalty() {
    for kind in [RouteKind::Local, RouteKind::Api] {
        cancel_case(kind, vec![], true, true).await;
    }
}

#[tokio::test(start_paused = true)]
async fn cancel_during_tool_call_does_not_replay_on_backup() {
    for kind in [RouteKind::Local, RouteKind::Api] {
        cancel_case(
            kind,
            vec![ProviderEvent::ToolCallStart {
                index: 0,
                id: "tool-1".into(),
                name: "net_fetch".into(),
            }],
            true,
            false,
        )
        .await;
    }
}
