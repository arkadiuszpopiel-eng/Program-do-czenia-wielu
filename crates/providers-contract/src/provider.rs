//! Trait `ModelProvider` (PLAN §5.1, ADR 0005) i typy pomocnicze.

use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::capabilities::{ModelInfo, ProviderCapabilities};
use crate::error::{ProviderError, ProviderErrorKind};
use crate::event::{ProviderEvent, Usage};
use crate::message::ProviderId;
use crate::pricing::{Cost, CostEstimate};
use crate::request::ChatRequest;

/// Strumień zdarzeń odpowiedzi. Kończy się dokładnie jednym zdarzeniem końcowym
/// (`Stop` albo `Error`), po którym zwraca `None`. Upuszczenie strumienia przerywa połączenie.
pub type ProviderStream = Pin<Box<dyn Stream<Item = ProviderEvent> + Send + 'static>>;

/// Stan zdrowia dostawcy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    /// Ostatnie wywołania udane.
    Healthy,
    /// Pojedyncze błędy — działa, ale Router może obniżyć priorytet.
    Degraded,
    /// Seria błędów lub błąd uwierzytelnienia — Router omija.
    Unavailable,
    /// Brak klucza — trasa niewidoczna dla Routera (PLAN §5.6).
    Unconfigured,
}

/// Migawka zdrowia (bez ruchu sieciowego; test połączenia = `list_models`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderHealth {
    /// Stan.
    pub state: HealthState,
    /// Liczba kolejnych nieudanych wywołań.
    pub consecutive_failures: u32,
    /// Rodzaj ostatniego błędu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<ProviderErrorKind>,
    /// Opóźnienie do pierwszego tokenu ostatniego udanego wywołania (ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_ttft_ms: Option<u64>,
}

impl ProviderHealth {
    /// Zdrowy, bez historii.
    pub fn healthy() -> Self {
        Self {
            state: HealthState::Healthy,
            consecutive_failures: 0,
            last_error: None,
            last_ttft_ms: None,
        }
    }

    /// Wylicza stan z historii wywołań: błąd `Auth` → `Unavailable`; ≥ `unavailable_after`
    /// kolejnych błędów → `Unavailable`; ≥ 1 → `Degraded`.
    pub fn from_stats(
        consecutive_failures: u32,
        last_error: Option<ProviderErrorKind>,
        last_ttft_ms: Option<u64>,
        unavailable_after: u32,
    ) -> Self {
        let state = match (&last_error, consecutive_failures) {
            (_, 0) => HealthState::Healthy,
            (Some(ProviderErrorKind::Auth), _) => HealthState::Unavailable,
            (_, n) if n >= unavailable_after => HealthState::Unavailable,
            _ => HealthState::Degraded,
        };
        Self {
            state,
            consecutive_failures,
            last_error,
            last_ttft_ms,
        }
    }
}

/// Żądanie osadzeń.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EmbeddingRequest {
    /// Model osadzeń.
    pub model: String,
    /// Teksty wejściowe.
    pub input: Vec<String>,
}

/// Odpowiedź z osadzeniami (w kolejności wejścia).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EmbeddingResponse {
    /// Wektory.
    pub vectors: Vec<Vec<f32>>,
    /// Zużycie (tylko wejście).
    pub usage: Usage,
}

/// Dostawca modeli: jednostką są tokeny (ADR 0005). Implementują `providers-api-impl`
/// (Anthropic, OpenAI, adapter generyczny), `providers-local` (llama.cpp) i `providers-fake`.
///
/// ```
/// # async fn demo(provider: &dyn providers_contract::ModelProvider) {
/// use futures_util::StreamExt;
/// use providers_contract::{ChatRequest, Message, ProviderEvent, TurnAccumulator};
/// use tokio_util::sync::CancellationToken;
///
/// let req = ChatRequest::new("claude-opus-5-5", vec![Message::user_text("Cześć!")]);
/// let cancel = CancellationToken::new(); // `cancel.cancel()` z barge-in kończy strumień ≤ 100 ms
/// let mut stream = provider.stream(req, cancel.clone());
/// let mut acc = TurnAccumulator::new(provider.id().clone());
/// while let Some(event) = stream.next().await {
///     if let ProviderEvent::TextDelta { text, .. } = &event {
///         print!("{text}");
///     }
///     acc.push(&event);
/// }
/// let turn = acc.finish(); // dopisz `turn.message` do historii (append-only)
/// # let _ = turn;
/// # }
/// ```
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Identyfikator dostawcy (wpis katalogu).
    fn id(&self) -> &ProviderId;

    /// Możliwości dostawcy i znanych modeli.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Strumień odpowiedzi. Nie blokuje: błędy walidacji, prywatności i sieci przychodzą jako
    /// zdarzenie `Error`. Anulowanie `cancel` kończy strumień zdarzeniem
    /// `Stop { reason: Cancelled }` i przerywa połączenie HTTP w ≤ 100 ms.
    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream;

    /// Migawka zdrowia (bez ruchu sieciowego).
    fn health(&self) -> ProviderHealth;

    /// Oszacowanie kosztu żądania z tabeli cen konfiguracji; `None` = brak cennika dla modelu.
    fn estimate_cost(&self, request: &ChatRequest) -> Option<CostEstimate>;

    /// Koszt faktycznego zużycia; `None` = brak cennika dla modelu.
    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost>;

    /// Lista modeli z Models API dostawcy (test połączenia w kreatorze kont).
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;

    /// Osadzenia. Domyślnie nieobsługiwane.
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::new(
            ProviderErrorKind::Unsupported,
            format!(
                "dostawca `{}` nie obsługuje osadzeń ({})",
                self.id(),
                request.model
            ),
        ))
    }
}

/// `Arc<P>` jest dostawcą (rejestr trzyma `Arc<dyn ModelProvider>`).
#[async_trait]
impl<P: ModelProvider + ?Sized> ModelProvider for std::sync::Arc<P> {
    fn id(&self) -> &ProviderId {
        (**self).id()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        (**self).capabilities()
    }

    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        (**self).stream(request, cancel)
    }

    fn health(&self) -> ProviderHealth {
        (**self).health()
    }

    fn estimate_cost(&self, request: &ChatRequest) -> Option<CostEstimate> {
        (**self).estimate_cost(request)
    }

    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost> {
        (**self).cost(model, usage)
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        (**self).list_models().await
    }

    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        (**self).embed(request).await
    }
}

/// Nazwy zdarzeń magistrali publikowanych przez moduły dostawców (SPEC providers-api).
pub mod events {
    /// Wywołanie rozpoczęte (dostawca, model, sesja).
    pub const CALL_STARTED: &str = "provider.call.started";
    /// Wywołanie zakończone (tokeny, koszt, opóźnienie, powód zatrzymania).
    pub const CALL_FINISHED: &str = "provider.call.finished";
    /// Błąd wywołania (429/5xx/timeout…).
    pub const ERROR: &str = "provider.error";
    /// Odmowa modelu (`stop_reason: refusal`).
    pub const REFUSAL: &str = "provider.refusal";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_from_stats() {
        assert_eq!(
            ProviderHealth::from_stats(0, None, None, 3).state,
            HealthState::Healthy
        );
        assert_eq!(
            ProviderHealth::from_stats(1, Some(ProviderErrorKind::Network), None, 3).state,
            HealthState::Degraded
        );
        assert_eq!(
            ProviderHealth::from_stats(3, Some(ProviderErrorKind::Network), None, 3).state,
            HealthState::Unavailable
        );
        assert_eq!(
            ProviderHealth::from_stats(1, Some(ProviderErrorKind::Auth), None, 3).state,
            HealthState::Unavailable
        );
        assert_eq!(ProviderHealth::healthy().consecutive_failures, 0);
    }
}
