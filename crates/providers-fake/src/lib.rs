//! Atrapa `ModelProvider` (docs/modules/providers-api/SPEC.md §Fake, PLAN §4.5).
//!
//! - **Skrypty** ([`Script`]): kolejka odpowiedzi na kolejne żądania + skrypt domyślny;
//! - **wirtualny zegar**: opóźnienia to `tokio::time::sleep` — w testach z
//!   `#[tokio::test(start_paused = true)]` czas płynie wirtualnie i deterministycznie;
//! - **wstrzykiwanie błędów**: [`FakeProvider::fail_next`], [`Script::http_error`], [`Script::stall`];
//! - **weryfikacja żądań**: [`FakeProvider::requests`] (po walidacji/prywatności/projekcji historii);
//! - **record/replay**: [`Recorder`] nagrywa kasety NDJSON z dowolnego dostawcy,
//!   [`FakeProvider::from_cassette`] je odtwarza.
//!
//! Tylko jako `dev-dependency` innych modułów (crates/README.md).
//!
//! ```
//! # #[tokio::main(flavor = "current_thread")] async fn main() {
//! use futures_util::StreamExt;
//! use providers_contract::{CancellationToken, ChatRequest, Message, ModelProvider};
//! use providers_fake::{FakeProvider, Script};
//!
//! let fake = FakeProvider::new("fake");
//! fake.push_script(Script::text("fake-model", &["Cześć", "!"]));
//! let req = ChatRequest::new("fake-model", vec![Message::user_text("hej")]);
//! let events: Vec<_> = fake.stream(req, CancellationToken::new()).collect().await;
//! assert_eq!(events.len(), 5); // Started, 2× TextDelta, Usage, Stop
//! assert_eq!(fake.requests().len(), 1);
//! # }
//! ```

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod cassette;
mod player;
mod script;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, EmbeddingRequest, EmbeddingResponse,
    InterruptionRendering, ModelCapabilities, ModelInfo, ModelProvider, Pricing, PricingTable,
    ProviderCapabilities, ProviderError, ProviderErrorKind, ProviderEvent, ProviderHealth,
    ProviderId, ProviderPrivacy, ProviderStream, StopReason, ThinkingSupport, Usage, check_privacy,
    project_history,
};

pub use cassette::{CassetteEntry, CassetteError, Recorder, TimedEvent, from_ndjson, to_ndjson};
pub use player::FakeTimeouts;
pub use script::{Script, Step, StreamSpeed};

use player::{Stats, lock};

/// Model domyślny atrapy.
pub const FAKE_MODEL: &str = "fake-model";

#[derive(Debug, Default)]
struct State {
    queue: VecDeque<Script>,
    default: Option<Script>,
    calls: Vec<ChatRequest>,
    wire: Vec<ChatRequest>,
}

/// Deterministyczny dostawca skryptowany.
#[derive(Debug, Clone)]
pub struct FakeProvider {
    id: ProviderId,
    models: BTreeMap<String, ModelCapabilities>,
    pricing: PricingTable,
    privacy: ProviderPrivacy,
    interruption: InterruptionRendering,
    timeouts: FakeTimeouts,
    state: Arc<Mutex<State>>,
    stats: Arc<Mutex<Stats>>,
}

impl FakeProvider {
    /// Atrapa z jednym modelem [`FAKE_MODEL`] (narzędzia, myślenie opcjonalne, wizja) i profilem `eu`/`EU`.
    pub fn new(id: &str) -> Self {
        let caps = ModelCapabilities {
            tools: true,
            strict_tools: true,
            forced_tool_choice: true,
            vision: true,
            thinking: ThinkingSupport::Optional,
            effort: true,
            sampling: true,
            prompt_cache: true,
            context_window: Some(32_000),
            max_output_tokens: Some(4_096),
            ..ModelCapabilities::default()
        };
        Self {
            id: ProviderId::new(id),
            models: BTreeMap::from([(FAKE_MODEL.to_owned(), caps)]),
            pricing: PricingTable::new(),
            privacy: ProviderPrivacy::new("eu", "EU"),
            interruption: InterruptionRendering::AppendNote,
            timeouts: FakeTimeouts::default(),
            state: Arc::default(),
            stats: Arc::default(),
        }
    }

    /// Ustawia profil prywatności (tag i jurysdykcja z katalogu).
    pub fn with_privacy(mut self, privacy: ProviderPrivacy) -> Self {
        self.privacy = privacy;
        self
    }

    /// Dodaje cennik modelu.
    pub fn with_pricing(mut self, model: &str, pricing: Pricing) -> Self {
        self.pricing.insert(model.to_owned(), pricing);
        self
    }

    /// Dodaje/zastępuje możliwości modelu.
    pub fn with_model(mut self, model: &str, caps: ModelCapabilities) -> Self {
        self.models.insert(model.to_owned(), caps);
        self
    }

    /// Ustawia limity czasu symulowane dla `Step::Stall`/`Step::Delay`.
    pub fn with_timeouts(mut self, timeouts: FakeTimeouts) -> Self {
        self.timeouts = timeouts;
        self
    }

    /// Strategia renderowania przerwanych tur.
    pub fn with_interruption(mut self, rendering: InterruptionRendering) -> Self {
        self.interruption = rendering;
        self
    }

    /// Skrypt używany, gdy kolejka jest pusta.
    pub fn with_default_script(self, script: Script) -> Self {
        lock(&self.state).default = Some(script);
        self
    }

    /// Dopisuje skrypt odpowiedzi na kolejne żądanie.
    pub fn push_script(&self, script: Script) {
        lock(&self.state).queue.push_back(script);
    }

    /// Następne żądanie zakończy się tym błędem (np. 429/5xx/timeout).
    pub fn fail_next(&self, error: ProviderError) {
        self.push_script(Script::error(error));
    }

    /// Żądania, które „dotarły do dostawcy" (po walidacji i prywatności; historia po projekcji).
    pub fn requests(&self) -> Vec<ChatRequest> {
        lock(&self.state).wire.clone()
    }

    /// Wszystkie wywołania `stream` (także odrzucone lokalnie).
    pub fn calls(&self) -> Vec<ChatRequest> {
        lock(&self.state).calls.clone()
    }

    /// Liczba skryptów w kolejce.
    pub fn pending_scripts(&self) -> usize {
        lock(&self.state).queue.len()
    }

    fn admit(&self, request: &ChatRequest) -> Result<Script, ProviderError> {
        request.validate()?;
        check_privacy(&request.meta.privacy, &self.privacy)?;
        let mut st = lock(&self.state);
        let mut wire = request.clone();
        wire.messages = project_history(&request.messages, self.interruption)
            .into_iter()
            .map(std::borrow::Cow::into_owned)
            .collect();
        st.wire.push(wire);
        st.queue
            .pop_front()
            .or_else(|| st.default.clone())
            .ok_or_else(|| ProviderError::new(ProviderErrorKind::Protocol, "atrapa: brak skryptu"))
    }
}

#[async_trait]
impl ModelProvider for FakeProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider: self.id.clone(),
            default_model: Some(FAKE_MODEL.to_owned()),
            models: self.models.clone(),
            interruption: self.interruption,
            privacy: self.privacy.clone(),
        }
    }

    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        lock(&self.state).calls.push(request.clone());
        if cancel.is_cancelled() {
            return player::single(ProviderEvent::stop(StopReason::Cancelled));
        }
        match self.admit(&request) {
            Ok(script) => player::play(script, cancel, self.timeouts, Arc::clone(&self.stats)),
            Err(e) => player::single(ProviderEvent::Error(e)),
        }
    }

    fn health(&self) -> ProviderHealth {
        let st = lock(&self.stats);
        ProviderHealth::from_stats(
            st.consecutive_failures,
            st.last_error.clone(),
            st.last_ttft_ms,
            3,
        )
    }

    fn estimate_cost(&self, request: &ChatRequest) -> Option<CostEstimate> {
        let pricing = self.pricing.get(&request.model)?;
        let max_out = request
            .params
            .max_tokens
            .or_else(|| {
                self.models
                    .get(&request.model)
                    .and_then(|c| c.max_output_tokens)
            })
            .unwrap_or(4_096);
        Some(pricing.estimate(request, u64::from(max_out)))
    }

    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost> {
        self.pricing.get(model).map(|p| p.cost(usage))
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(self
            .models
            .iter()
            .map(|(id, caps)| ModelInfo {
                id: id.clone(),
                display_name: None,
                created: None,
                capabilities: Some(caps.clone()),
            })
            .collect())
    }

    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        let vectors = request
            .input
            .iter()
            .map(|text| pseudo_embedding(text))
            .collect();
        let tokens = request
            .input
            .iter()
            .map(|t| t.len().div_ceil(4) as u64)
            .sum();
        Ok(EmbeddingResponse {
            vectors,
            usage: Usage {
                input_tokens: tokens,
                ..Usage::default()
            },
        })
    }
}

/// Deterministyczny wektor 8-wymiarowy (FNV-1a po bajtach, znormalizowany).
fn pseudo_embedding(text: &str) -> Vec<f32> {
    let mut v = [0f32; 8];
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (i, b) in text.bytes().enumerate() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        #[allow(clippy::cast_precision_loss)]
        let x = (h % 1000) as f32 / 1000.0;
        v[i % 8] += x;
    }
    let norm = v
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt()
        .max(f32::EPSILON);
    v.iter().map(|x| x / norm).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_is_deterministic_and_normalized() {
        let a = pseudo_embedding("zażółć");
        assert_eq!(a, pseudo_embedding("zażółć"));
        assert_ne!(a, pseudo_embedding("gęślą"));
        let norm: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
        assert!(pseudo_embedding("").iter().all(|x| *x == 0.0));
    }
}
