//! Wspólny adapter prób: rzeczywisty LocalProvider, limit96/temp0, bez podmiany odpowiedzi.
#![allow(clippy::unwrap_used)]
use app_core::ports::{BrainChoice, BrainError, BrainPort, BrainRequest};
use async_trait::async_trait;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, ModelInfo, ModelProvider,
    ProviderCapabilities, ProviderError, ProviderHealth, ProviderId, ProviderStream, Usage,
};
use providers_local_impl::LocalProvider;
use std::sync::{Arc, Mutex};

pub(super) struct LocalBrain {
    pub(super) provider: Arc<dyn ModelProvider>,
    pub(super) model: String,
    pub(super) seen: Mutex<Vec<(String, ChatRequest)>>,
    pub(super) sent: Arc<Mutex<Vec<ChatRequest>>>,
}

/// Jawny budżet tej próby integracji; nie zmienia ustawień produkcyjnego adaptera.
pub(super) struct BoundedProvider(
    pub(super) LocalProvider,
    pub(super) Arc<Mutex<Vec<ChatRequest>>>,
);

#[async_trait]
impl ModelProvider for BoundedProvider {
    fn id(&self) -> &ProviderId {
        self.0.id()
    }
    fn capabilities(&self) -> ProviderCapabilities {
        self.0.capabilities()
    }
    fn stream(&self, mut request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        request.params.max_tokens = Some(96);
        request.params.temperature = Some(0.0);
        self.1.lock().unwrap().push(request.clone());
        self.0.stream(request, cancel)
    }
    fn health(&self) -> ProviderHealth {
        self.0.health()
    }
    fn estimate_cost(&self, request: &ChatRequest) -> Option<CostEstimate> {
        self.0.estimate_cost(request)
    }
    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost> {
        self.0.cost(model, usage)
    }
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.0.list_models().await
    }
}

#[async_trait]
impl BrainPort for LocalBrain {
    async fn choose(&self, req: &BrainRequest) -> Result<BrainChoice, BrainError> {
        if let Some(chat) = &req.chat {
            self.seen
                .lock()
                .unwrap()
                .push((req.agent.clone(), chat.clone()));
        }
        Ok(BrainChoice {
            provider: self.provider.clone(),
            provider_id: "local".into(),
            provider_name: "llama.cpp (CPU)".into(),
            account: None,
            model: self.model.clone(),
            context_window: Some(8192),
            routed: false,
            route: None,
        })
    }
    fn keys_configured(&self) -> bool {
        false
    }
}
