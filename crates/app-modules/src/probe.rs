//! Test połączenia i wykrywanie modeli dla `accounts-hub` przez adaptery `providers-api-impl`
//! (`list_models` = test klucza, SPEC providers-api). Klucz nie opuszcza tego wywołania.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use accounts_hub_contract::{
    ConnectionReport, ConnectionRequest, ConnectionTester, ModelId, ModelInfo, ModelListError,
    ModelLister, TestOutcome,
};
use async_trait::async_trait;
use providers_api_impl::{AccountProfile, CatalogEntry, build_provider};
use providers_contract::{ApiKey, ModelProvider, ProviderError, ProviderErrorKind, StaticKey};

/// Sonda dostawców na wpisach katalogu.
pub struct ProviderProbe {
    catalog: Arc<BTreeMap<String, CatalogEntry>>,
}

impl ProviderProbe {
    /// Sonda na katalogu adapterów.
    pub fn new(catalog: Arc<BTreeMap<String, CatalogEntry>>) -> Self {
        Self { catalog }
    }

    fn provider(&self, request: &ConnectionRequest<'_>) -> Result<Arc<dyn ModelProvider>, String> {
        let id = request.provider.id.as_str();
        let entry = self
            .catalog
            .get(id)
            .ok_or_else(|| format!("brak adaptera dla dostawcy `{id}`"))?;
        let key = StaticKey(request.secret.map(|s| ApiKey::new(s.expose_secret())));
        let mut profile = AccountProfile::new(Arc::new(key));
        profile.base_url = request.base_url.map(str::to_owned);
        build_provider(entry, profile).map_err(|e| e.to_string())
    }
}

fn outcome(error: &ProviderError) -> TestOutcome {
    match &error.kind {
        ProviderErrorKind::Auth => TestOutcome::InvalidKey,
        ProviderErrorKind::RateLimited { .. } => TestOutcome::RateLimited,
        ProviderErrorKind::Timeout { .. } => TestOutcome::Timeout,
        ProviderErrorKind::Unsupported => TestOutcome::Unsupported {
            message: error.message.clone(),
        },
        _ => TestOutcome::Network {
            message: error.to_string(),
        },
    }
}

#[async_trait]
impl ConnectionTester for ProviderProbe {
    async fn test(&self, request: ConnectionRequest<'_>) -> ConnectionReport {
        let provider = match self.provider(&request) {
            Ok(p) => p,
            Err(message) => {
                return ConnectionReport {
                    outcome: TestOutcome::Unsupported { message },
                    latency_ms: None,
                };
            }
        };
        let started = Instant::now();
        let result = provider.list_models().await;
        let latency_ms = u64::try_from(started.elapsed().as_millis()).ok();
        match result {
            Ok(_) => ConnectionReport {
                outcome: TestOutcome::Ok,
                latency_ms,
            },
            Err(e) => ConnectionReport {
                outcome: outcome(&e),
                latency_ms,
            },
        }
    }
}

#[async_trait]
impl ModelLister for ProviderProbe {
    async fn list_models(
        &self,
        request: ConnectionRequest<'_>,
    ) -> Result<Vec<ModelInfo>, ModelListError> {
        let provider = self
            .provider(&request)
            .map_err(|_| ModelListError::Unsupported)?;
        let models = provider.list_models().await.map_err(|e| match e.kind {
            ProviderErrorKind::Auth => ModelListError::InvalidKey,
            ProviderErrorKind::Timeout { .. } => ModelListError::Timeout,
            ProviderErrorKind::Unsupported => ModelListError::Unsupported,
            _ => ModelListError::Network {
                message: e.to_string(),
            },
        })?;
        Ok(models
            .into_iter()
            .filter_map(|m| {
                Some(ModelInfo {
                    id: ModelId::new(m.id).ok()?,
                    display_name: m.display_name,
                    capabilities: None,
                    context_window: m.capabilities.and_then(|c| c.context_window),
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_errors_map_to_test_outcomes() {
        let e = |kind| ProviderError::new(kind, "x");
        assert_eq!(
            outcome(&e(ProviderErrorKind::Auth)),
            TestOutcome::InvalidKey
        );
        assert_eq!(
            outcome(&e(ProviderErrorKind::RateLimited {
                retry_after_ms: None
            })),
            TestOutcome::RateLimited
        );
        assert!(matches!(
            outcome(&e(ProviderErrorKind::Network)),
            TestOutcome::Network { .. }
        ));
    }
}
