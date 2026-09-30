//! Publikacja zdarzeń wywołań na magistralę (SPEC: `provider.call.started/finished`,
//! `provider.error`, `provider.refusal`). Ładunki zawierają wyłącznie metadane — nigdy treść
//! rozmowy ani klucze.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, EventKind, Level, SessionId};
use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, EmbeddingRequest, EmbeddingResponse,
    ModelInfo, ModelProvider, ProviderCapabilities, ProviderError, ProviderEvent, ProviderHealth,
    ProviderId, ProviderStream, StopReason, Usage, events,
};
use serde_json::json;
use tokio::time::Instant;

/// Dostawca opakowany publikacją zdarzeń na magistralę.
pub struct ObservedProvider {
    inner: Arc<dyn ModelProvider>,
    bus: Arc<dyn EventBus>,
}

impl ObservedProvider {
    /// Opakowuje dostawcę.
    pub fn new(inner: Arc<dyn ModelProvider>, bus: Arc<dyn EventBus>) -> Self {
        Self { inner, bus }
    }
}

struct Observe {
    events: ProviderStream,
    meta: Meta,
}

/// Stan obserwacji bez strumienia (musi być `Sync`, bo jest pożyczany przez `await`).
struct Meta {
    inner: Arc<dyn ModelProvider>,
    bus: Arc<dyn EventBus>,
    provider: String,
    model: String,
    session: Option<String>,
    started: bool,
    t0: Instant,
    ttft: Option<Duration>,
    usage: Usage,
}

fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

impl Meta {
    async fn publish(
        &self,
        kind: EventKind,
        level: Level,
        payload: serde_json::Value,
        cost: Option<core_bus_contract::Cost>,
    ) {
        let mut event = Event::new(kind, level, payload);
        if let Some(s) = &self.session {
            event = event.with_session(SessionId::new(s.clone()));
        }
        if let Some(c) = cost {
            event = event.with_cost(c);
        }
        // Zdarzenia są diagnostyczne: błąd magistrali nie przerywa odpowiedzi.
        let _ = self.bus.publish(event).await;
    }

    async fn on_terminal(&self, ev: &ProviderEvent) {
        let latency = ms(self.t0.elapsed());
        match ev {
            ProviderEvent::Stop { reason, details } => {
                let cost: Option<Cost> = self.inner.cost(&self.model, &self.usage);
                let payload = json!({
                    "name": events::CALL_FINISHED, "provider": self.provider, "model": self.model,
                    "stop": reason, "usage": self.usage, "ttft_ms": self.ttft.map(ms),
                    "latency_ms": latency, "cost_nano_usd": cost.map(|c| c.nano_usd),
                });
                let bus_cost = core_bus_contract::Cost {
                    input_tokens: self.usage.total_input(),
                    output_tokens: self.usage.output_tokens,
                    micro_usd: cost.map_or(0, Cost::micro_usd_ceil),
                    latency_ms: Some(latency),
                };
                self.publish(EventKind::ModelCall, Level::Info, payload, Some(bus_cost))
                    .await;
                if *reason == StopReason::Refusal {
                    let category = details.as_ref().and_then(|d| d.category.clone());
                    let payload = json!({"provider": self.provider, "model": self.model, "category": category});
                    self.publish(
                        EventKind::Custom(events::REFUSAL.into()),
                        Level::Warn,
                        payload,
                        None,
                    )
                    .await;
                }
            }
            ProviderEvent::Error(e) => {
                let payload = json!({
                    "provider": self.provider, "model": self.model, "kind": e.kind, "status": e.status,
                    "after_output": e.after_output, "fallback": e.should_fallback(), "latency_ms": latency,
                });
                self.publish(
                    EventKind::Custom(events::ERROR.into()),
                    Level::Warn,
                    payload,
                    None,
                )
                .await;
            }
            _ => {}
        }
    }
}

impl Observe {
    async fn next(&mut self) -> Option<ProviderEvent> {
        let meta = &mut self.meta;
        if !meta.started {
            meta.started = true;
            let payload = json!({"name": events::CALL_STARTED, "provider": meta.provider, "model": meta.model});
            meta.publish(EventKind::ModelCall, Level::Debug, payload, None)
                .await;
        }
        let ev = self.events.next().await?;
        let meta = &mut self.meta;
        match &ev {
            ProviderEvent::Started { model, .. } if !model.is_empty() => {
                meta.model.clone_from(model)
            }
            ProviderEvent::Usage(u) => meta.usage = *u,
            e if e.is_content() && meta.ttft.is_none() => meta.ttft = Some(meta.t0.elapsed()),
            e if e.is_terminal() => meta.on_terminal(e).await,
            _ => {}
        }
        Some(ev)
    }
}

#[async_trait]
impl ModelProvider for ObservedProvider {
    fn id(&self) -> &ProviderId {
        self.inner.id()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        let meta = Meta {
            provider: self.inner.id().to_string(),
            model: request.model.clone(),
            session: request.meta.session.clone(),
            inner: Arc::clone(&self.inner),
            bus: Arc::clone(&self.bus),
            started: false,
            t0: Instant::now(),
            ttft: None,
            usage: Usage::default(),
        };
        let state = Observe {
            events: self.inner.stream(request, cancel),
            meta,
        };
        Box::pin(futures_util::stream::unfold(state, |mut st| async move {
            st.next().await.map(|ev| (ev, st))
        }))
    }

    fn health(&self) -> ProviderHealth {
        self.inner.health()
    }

    fn estimate_cost(&self, request: &ChatRequest) -> Option<CostEstimate> {
        self.inner.estimate_cost(request)
    }

    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost> {
        self.inner.cost(model, usage)
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.inner.list_models().await
    }

    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        self.inner.embed(request).await
    }
}
