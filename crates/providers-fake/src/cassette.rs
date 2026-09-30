//! Kasety NDJSON: nagrywanie odpowiedzi dowolnego dostawcy i ich odtwarzanie w atrapie.
//!
//! Format: jedna linia = jedna odpowiedź `{"model": "...", "events": [{"at_ms": 12, "event": {...}}]}`,
//! gdzie `event` to [`ProviderEvent`] w schemacie IR (`providers_contract::provider_event_schema`).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, EmbeddingRequest, EmbeddingResponse,
    ModelInfo, ModelProvider, ProviderCapabilities, ProviderError, ProviderEvent, ProviderHealth,
    ProviderId, ProviderStream, Usage,
};
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use crate::FakeProvider;
use crate::player::lock;
use crate::script::{Script, Step};

/// Zdarzenie z czasem od początku odpowiedzi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedEvent {
    /// Milisekundy od wywołania `stream`.
    pub at_ms: u64,
    /// Zdarzenie.
    pub event: ProviderEvent,
}

/// Jedna nagrana odpowiedź.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CassetteEntry {
    /// Model z żądania.
    pub model: String,
    /// Zdarzenia w kolejności.
    pub events: Vec<TimedEvent>,
}

impl CassetteEntry {
    /// Skrypt odtwarzający odpowiedź z zachowaniem odstępów czasu.
    pub fn to_script(&self) -> Script {
        let mut steps = Vec::with_capacity(self.events.len() * 2);
        let mut last = 0u64;
        for te in &self.events {
            if te.at_ms > last {
                steps.push(Step::Delay(Duration::from_millis(te.at_ms - last)));
                last = te.at_ms;
            }
            steps.push(Step::Emit(te.event.clone()));
        }
        Script::new(steps)
    }
}

/// Błąd kasety.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CassetteError {
    /// Linia nie jest poprawnym wpisem.
    #[error("linia {line} kasety: {message}")]
    Parse {
        /// Numer linii (od 1).
        line: usize,
        /// Opis.
        message: String,
    },
}

/// Serializuje wpisy do NDJSON.
pub fn to_ndjson(entries: &[CassetteEntry]) -> String {
    entries
        .iter()
        .filter_map(|e| serde_json::to_string(e).ok())
        .map(|line| line + "\n")
        .collect()
}

/// Parsuje NDJSON (puste linie pomijane).
pub fn from_ndjson(text: &str) -> Result<Vec<CassetteEntry>, CassetteError> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            serde_json::from_str(l).map_err(|e| CassetteError::Parse {
                line: i + 1,
                message: e.to_string(),
            })
        })
        .collect()
}

impl FakeProvider {
    /// Atrapa odtwarzająca kasetę: kolejne żądania dostają kolejne wpisy.
    pub fn from_cassette(id: &str, ndjson: &str) -> Result<Self, CassetteError> {
        let fake = Self::new(id);
        for entry in from_ndjson(ndjson)? {
            fake.push_script(entry.to_script());
        }
        Ok(fake)
    }
}

/// Nagrywa odpowiedzi opakowanego dostawcy (np. adaptera na żywo z kluczem) do kasety.
pub struct Recorder<P> {
    inner: P,
    entries: Arc<Mutex<Vec<CassetteEntry>>>,
}

impl<P: ModelProvider> Recorder<P> {
    /// Opakowuje dostawcę.
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            entries: Arc::default(),
        }
    }

    /// Nagrane wpisy (tylko zakończone odpowiedzi).
    pub fn entries(&self) -> Vec<CassetteEntry> {
        lock(&self.entries).clone()
    }

    /// Kaseta NDJSON.
    pub fn cassette_ndjson(&self) -> String {
        to_ndjson(&self.entries())
    }
}

#[async_trait]
impl<P: ModelProvider> ModelProvider for Recorder<P> {
    fn id(&self) -> &ProviderId {
        self.inner.id()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        let model = request.model.clone();
        let start = Instant::now();
        let sink = Arc::clone(&self.entries);
        let mut buffer: Vec<TimedEvent> = Vec::new();
        let inner = self.inner.stream(request, cancel);
        Box::pin(inner.map(move |event| {
            let at_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
            buffer.push(TimedEvent {
                at_ms,
                event: event.clone(),
            });
            if event.is_terminal() {
                lock(&sink).push(CassetteEntry {
                    model: model.clone(),
                    events: std::mem::take(&mut buffer),
                });
            }
            event
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndjson_round_trip_and_errors() {
        let entry = CassetteEntry {
            model: "m".into(),
            events: vec![
                TimedEvent {
                    at_ms: 0,
                    event: ProviderEvent::TextDelta {
                        index: 0,
                        text: "a".into(),
                    },
                },
                TimedEvent {
                    at_ms: 30,
                    event: ProviderEvent::stop(providers_contract::StopReason::EndTurn),
                },
            ],
        };
        let text = to_ndjson(&[entry.clone(), entry.clone()]);
        assert_eq!(text.lines().count(), 2);
        assert_eq!(
            from_ndjson(&format!("\n{text}")).unwrap(),
            vec![entry.clone(), entry.clone()]
        );
        let script = entry.to_script();
        assert_eq!(script.steps[1], Step::Delay(Duration::from_millis(30)));
        assert!(matches!(
            from_ndjson("{zepsute"),
            Err(CassetteError::Parse { line: 1, .. })
        ));
    }
}
