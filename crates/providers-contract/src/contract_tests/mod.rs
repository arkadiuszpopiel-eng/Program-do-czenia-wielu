//! Współdzielone testy kontraktowe `ModelProvider` (feature `contract-tests`).
//!
//! Ten sam zestaw uruchamia `providers-fake` (skrypty zdarzeń, wirtualny zegar) i każdy adapter
//! z `providers-api-impl` (serwer fixture z nagraniami SSE zgodnymi z dokumentacją API).
//! Implementacja dostarcza [`Harness`], który programuje dostawcę neutralnym [`Scenario`].

use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::time::Instant;

use crate::event::ProviderEvent;
use crate::pricing::Pricing;
use crate::provider::{ModelProvider, ProviderStream};

mod cases;
mod failures;

pub use cases::{
    cost_comes_from_pricing, interrupted_turn_is_rendered_append_only, text_stream_follows_grammar,
    thinking_is_signed_and_replayed, tool_call_is_assembled,
};
pub use failures::*;

/// Neutralny scenariusz odpowiedzi dostawcy.
#[derive(Debug, Clone, PartialEq)]
pub enum Scenario {
    /// Odpowiedź tekstowa w podanych fragmentach; `Stop(EndTurn)`; `Usage` przed końcem.
    Text {
        /// Fragmenty tekstu (w kolejności).
        chunks: Vec<String>,
    },
    /// Jedno wywołanie narzędzia; `Stop(ToolUse)`.
    ToolCall {
        /// Identyfikator wywołania.
        id: String,
        /// Nazwa narzędzia.
        name: String,
        /// Argumenty (obiekt).
        arguments: serde_json::Value,
    },
    /// Blok myślenia z podpisem, potem tekst (tylko dostawcy z podpisanym myśleniem).
    Thinking {
        /// Tekst myślenia.
        thinking: String,
        /// Podpis.
        signature: String,
        /// Tekst odpowiedzi.
        text: String,
    },
    /// Odpowiedź HTTP z błędem (przed strumieniem).
    HttpError {
        /// Kod HTTP.
        status: u16,
        /// Nagłówek `retry-after` w sekundach.
        retry_after_s: Option<u64>,
    },
    /// Odmowa modelu (`Stop(Refusal)`).
    Refusal,
    /// Ucięcie na `max_tokens`.
    MaxTokens {
        /// Tekst przed ucięciem.
        text: String,
    },
    /// Dostawca milczy (brak zdarzeń) — oczekiwany `Timeout`.
    Stall,
    /// Wolny strumień do testu anulowania.
    Slow {
        /// Fragmenty.
        chunks: Vec<String>,
        /// Odstęp między fragmentami.
        interval: Duration,
    },
}

/// Uchwyt testowy implementacji (atrapy albo adaptera z serwerem fixture).
#[async_trait]
pub trait Harness: Send + Sync {
    /// Typ dostawcy.
    type Provider: ModelProvider + 'static;

    /// Świeży dostawca, którego każde wywołanie `stream` odtwarza `scenario`.
    /// `None` = scenariusz nie dotyczy dostawcy (przypadek pominięty).
    async fn provider(&self, scenario: Scenario) -> Option<Self::Provider>;

    /// Dostawca z profilem prywatności blokującym sesje prywatne (np. `cn-may-train`/`CN`).
    async fn provider_blocking_private(&self) -> Self::Provider;

    /// Liczba żądań, które dotarły „na drut" do ostatnio utworzonego dostawcy.
    fn wire_requests(&self) -> usize;

    /// Treść ostatniego żądania na drucie (JSON ciała albo serializacja IR).
    fn last_wire_request(&self) -> Option<String>;

    /// Model używany w żądaniach testowych (ma wpis w cenniku [`Harness::pricing`]).
    fn model(&self) -> String;

    /// Cennik skonfigurowany dla [`Harness::model`].
    fn pricing(&self) -> Pricing;

    /// Limit czasu (first-token/idle) skonfigurowany w dostawcy dla scenariusza `Stall`.
    fn stall_timeout(&self) -> Duration;
}

/// Maksymalny czas od błędu dostawcy do sklasyfikowanego zdarzenia `Error` (ACC F1-04: fallback ≤ 2 s).
pub const FALLBACK_BUDGET: Duration = Duration::from_secs(2);

/// Maksymalny czas od anulowania do zdarzenia końcowego (SPEC providers-api).
pub const CANCEL_BUDGET: Duration = Duration::from_millis(100);

/// Zebrany strumień: zdarzenia z czasem od startu.
#[derive(Debug)]
pub struct Collected {
    /// Zdarzenia z czasem nadejścia.
    pub events: Vec<(Duration, ProviderEvent)>,
}

impl Collected {
    /// Same zdarzenia.
    pub fn plain(&self) -> impl Iterator<Item = &ProviderEvent> {
        self.events.iter().map(|(_, e)| e)
    }

    /// Zdarzenie końcowe (ostatnie).
    pub fn terminal(&self) -> &ProviderEvent {
        match self.events.last() {
            Some((_, ev)) => ev,
            None => panic!("pusty strumień — brak zdarzenia końcowego"),
        }
    }

    /// Czas nadejścia zdarzenia końcowego.
    pub fn terminal_at(&self) -> Duration {
        self.events.last().map_or(Duration::ZERO, |(t, _)| *t)
    }

    /// Sklejony tekst z `TextDelta`.
    pub fn text(&self) -> String {
        self.plain()
            .filter_map(|e| match e {
                ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// Zbiera strumień do końca (limit bezpieczeństwa 10 s) i sprawdza gramatykę: dokładnie jedno
/// zdarzenie końcowe, zawsze ostatnie; `Started` przed pierwszą treścią.
pub async fn collect(mut stream: ProviderStream) -> Collected {
    let start = Instant::now();
    let mut events = Vec::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(10), stream.next()).await {
            Ok(Some(ev)) => events.push((start.elapsed(), ev)),
            Ok(None) => break,
            Err(_) => panic!("strumień nie zakończył się w 10 s; zdarzenia: {events:?}"),
        }
    }
    assert_grammar(&events);
    Collected { events }
}

fn assert_grammar(events: &[(Duration, ProviderEvent)]) {
    let terminals = events.iter().filter(|(_, e)| e.is_terminal()).count();
    assert_eq!(
        terminals, 1,
        "dokładnie jedno zdarzenie końcowe: {events:?}"
    );
    assert!(
        events.last().is_some_and(|(_, e)| e.is_terminal()),
        "zdarzenie końcowe musi być ostatnie: {events:?}"
    );
    let first_content = events.iter().position(|(_, e)| e.is_content());
    let started = events
        .iter()
        .position(|(_, e)| matches!(e, ProviderEvent::Started { .. }));
    if let Some(content) = first_content {
        assert!(
            started.is_some_and(|s| s < content),
            "`Started` musi poprzedzać treść: {events:?}"
        );
    }
}

/// Uruchamia cały zestaw przypadków na uchwycie.
pub async fn run_all<H: Harness>(h: &H) {
    text_stream_follows_grammar(h).await;
    tool_call_is_assembled(h).await;
    thinking_is_signed_and_replayed(h).await;
    http_errors_are_classified(h).await;
    refusal_is_a_stop_not_an_error(h).await;
    max_tokens_is_reported(h).await;
    stall_times_out_within_budget(h).await;
    cancel_ends_stream_fast(h).await;
    pre_cancelled_sends_nothing(h).await;
    private_request_is_blocked_before_wire(h).await;
    invalid_request_is_rejected_locally(h).await;
    interrupted_turn_is_rendered_append_only(h).await;
    cost_comes_from_pricing(h).await;
}
