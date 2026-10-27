//! Oddzielna trasa każdego konta tego samego dostawcy (klucz i limity pozostają w adapterze).
use crate::*;
use async_trait::async_trait;
use std::sync::Arc;
/// Provider route alias retaining the original policy and billing identity.
pub struct ProviderAlias {
    id: ProviderId,
    inner: Arc<dyn ModelProvider>,
}
impl ProviderAlias {
    /// Wrap only when a distinct route id is required.
    pub fn wrap(id: &str, inner: Arc<dyn ModelProvider>) -> Arc<dyn ModelProvider> {
        if inner.id().as_str() == id {
            inner
        } else {
            Arc::new(Self {
                id: ProviderId::new(id),
                inner,
            })
        }
    }
}
#[async_trait]
impl ModelProvider for ProviderAlias {
    fn policy_id(&self) -> &ProviderId {
        self.inner.policy_id()
    }
    fn id(&self) -> &ProviderId {
        &self.id
    }
    fn capabilities(&self) -> ProviderCapabilities {
        let mut c = self.inner.capabilities();
        c.provider = self.id.clone();
        c
    }
    fn model_capabilities(&self, model: &str) -> Option<crate::ModelCapabilities> {
        self.inner.model_capabilities(model)
    }
    fn stream(&self, r: ChatRequest, c: CancellationToken) -> ProviderStream {
        self.inner.stream(r, c)
    }
    fn health(&self) -> ProviderHealth {
        self.inner.health()
    }
    fn estimate_cost(&self, r: &ChatRequest) -> Option<CostEstimate> {
        self.inner.estimate_cost(r)
    }
    fn cost(&self, m: &str, u: &Usage) -> Option<Cost> {
        self.inner.cost(m, u)
    }
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.inner.list_models().await
    }
    fn can_escalate(&self, from: &str) -> bool {
        self.inner.can_escalate(from)
    }
    async fn check_key(&self) -> Result<Option<String>, ProviderError> {
        self.inner.check_key().await
    }
    async fn embed(&self, r: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        self.inner.embed(r).await
    }
}
