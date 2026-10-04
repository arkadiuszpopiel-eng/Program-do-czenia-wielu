//! Symptomy dostawców dla Diagnosty (przeniesione z `app-core`, limit rozmiaru crate'a): dostawca
//! wybrany dla tury bez Routera (czat i przebieg agentki) jest opakowany podsłuchem strumienia —
//! błąd HTTP (401/403 klucz, 429 limit, 5xx/529 awaria) → zdarzenie magistrali
//! `diagnostics.symptom` (moduł `providers-api`, cel = identyfikator dostawcy). Bez treści rozmowy
//! i komunikatu dostawcy — tylko kod.

use std::sync::Arc;

use app_api::ports::BrainChoice;
use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, EventKind, Level};
use diagnostician_contract::{EVENT_SYMPTOM, Symptom};
use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, EmbeddingRequest, EmbeddingResponse,
    ModelInfo, ModelProvider, ProviderCapabilities, ProviderError, ProviderErrorKind,
    ProviderEvent, ProviderHealth, ProviderId, ProviderStream, Usage,
};

/// Kod HTTP odpowiadający rodzajowi błędu (`None` — błąd bez znaczenia dla napraw).
fn status(kind: &ProviderErrorKind) -> Option<u16> {
    match kind {
        ProviderErrorKind::Auth => Some(401),
        ProviderErrorKind::RateLimited { .. } => Some(429),
        ProviderErrorKind::Overloaded { .. } => Some(529),
        ProviderErrorKind::Server { status } => Some(*status),
        _ => None,
    }
}

/// Zdarzenie symptomu (kod z odpowiedzi, gdy był; inaczej typowy dla rodzaju).
fn symptom_event(target: &str, error: &ProviderError) -> Option<Event> {
    let code = status(&error.kind).map(|typical| error.status.unwrap_or(typical))?;
    let symptom = serde_json::to_value(Symptom::Http { status: code }).ok()?;
    Some(Event::new(
        EventKind::Custom(EVENT_SYMPTOM.into()),
        Level::Warn,
        serde_json::json!({ "module": "providers-api", "symptom": symptom, "target": target }),
    ))
}

/// Dostawca z podsłuchem błędów strumienia.
struct Tap {
    inner: Arc<dyn ModelProvider>,
    bus: Arc<dyn EventBus>,
    target: String,
}

#[async_trait]
impl ModelProvider for Tap {
    fn id(&self) -> &ProviderId {
        self.inner.id()
    }
    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }
    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        let (bus, target) = (self.bus.clone(), self.target.clone());
        Box::pin(self.inner.stream(request, cancel).inspect(move |event| {
            let ProviderEvent::Error(e) = event else {
                return;
            };
            let (Some(ev), Ok(handle)) = (
                symptom_event(&target, e),
                tokio::runtime::Handle::try_current(),
            ) else {
                return;
            };
            let bus = bus.clone();
            // Zgłoszenie diagnostyczne — błąd magistrali nie wpływa na turę.
            handle.spawn(async move {
                let _ = bus.publish(ev).await;
            });
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

/// Opakowuje dostawcę wybranego dla tury podsłuchem symptomów. Wybór Routera (dekorator
/// z fallbackiem między dostawcami) zostaje bez zmian — cel symptomu byłby niejednoznaczny.
pub fn symptom_tap(mut choice: BrainChoice, bus: &Arc<dyn EventBus>) -> BrainChoice {
    if choice.routed {
        return choice;
    }
    choice.provider = Arc::new(Tap {
        inner: choice.provider,
        bus: bus.clone(),
        target: choice.provider_id.clone(),
    });
    choice
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_codes_for_repairable_errors_only() {
        assert_eq!(status(&ProviderErrorKind::Auth), Some(401));
        let limited = ProviderErrorKind::RateLimited {
            retry_after_ms: None,
        };
        assert_eq!(status(&limited), Some(429));
        assert_eq!(
            status(&ProviderErrorKind::Server { status: 503 }),
            Some(503)
        );
        assert_eq!(status(&ProviderErrorKind::InvalidRequest), None);
        assert_eq!(status(&ProviderErrorKind::Network), None);
    }

    #[test]
    fn event_carries_code_and_target_only() {
        let e = ProviderError::new(ProviderErrorKind::Auth, "klucz sk-tajny odrzucony")
            .with_status(403);
        let ev = symptom_event("anthropic", &e).unwrap();
        let text = ev.payload.to_string();
        assert!(text.contains("403") && text.contains("anthropic"));
        assert!(!text.contains("sk-tajny"), "bez komunikatu dostawcy");
        let none = ProviderError::new(ProviderErrorKind::Network, "x");
        assert!(symptom_event("anthropic", &none).is_none());
    }
}
