//! `LocalProvider` — `ModelProvider` nad sidecarem `llama-server` (klasa A, ADR 0014).
//!
//! `Started` jest emitowane od razu po przyjęciu żądania (Router nie traktuje zimnego startu
//! modelu jako milczenia dostawcy); zdarzenia silnika bez własnego `Started`. Koszt = 0,
//! zużycie tokenów raportowane (statystyki `cost-meter`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, HealthState, InterruptionRendering,
    ModelInfo, ModelProvider, Pricing, ProviderCapabilities, ProviderError, ProviderErrorKind,
    ProviderEvent, ProviderHealth, ProviderId, ProviderStream, RequestPrivacy, StopReason, Usage,
    check_privacy,
};

use crate::download::installed;
use crate::error::LocalError;
use crate::manifest::ModelEntry;
use crate::sidecar::Sidecar;

const FREE: Pricing = Pricing {
    input_per_mtok_usd: 0.0,
    output_per_mtok_usd: 0.0,
    cache_read_per_mtok_usd: None,
    cache_write_per_mtok_usd: None,
};

fn single(event: ProviderEvent) -> ProviderStream {
    Box::pin(futures_util::stream::iter([event]))
}

/// Dostawca lokalny.
pub struct LocalProvider {
    id: ProviderId,
    models: Vec<ModelEntry>,
    sidecar: Arc<Sidecar>,
}

/// Znacznik żądania w toku (zwalnia licznik przy upuszczeniu strumienia).
struct Busy(Arc<Sidecar>);

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.end();
    }
}

impl LocalProvider {
    /// Dostawca z manifestem modeli i menedżerem sidecara.
    pub fn new(models: Vec<ModelEntry>, sidecar: Arc<Sidecar>) -> Self {
        Self {
            id: ProviderId::new(sidecar.config().provider_id.as_str()),
            models,
            sidecar,
        }
    }

    /// Menedżer sidecara.
    pub fn sidecar(&self) -> &Arc<Sidecar> {
        &self.sidecar
    }

    /// Modele z manifestu.
    pub fn models(&self) -> &[ModelEntry] {
        &self.models
    }

    /// Wpis modelu.
    pub fn entry(&self, id: &str) -> Option<&ModelEntry> {
        self.models.iter().find(|m| m.id == id)
    }

    /// Ścieżka pliku modelu.
    pub fn model_path(&self, entry: &ModelEntry) -> PathBuf {
        self.sidecar.config().models_dir.join(&entry.file)
    }

    /// Zainstalowane modele (plik + zapisany hash).
    pub fn installed(&self) -> Vec<&ModelEntry> {
        let dir = &self.sidecar.config().models_dir;
        self.models.iter().filter(|m| installed(dir, m)).collect()
    }

    fn admit(&self, req: &ChatRequest) -> Result<ModelEntry, LocalError> {
        let entry = self
            .entry(&req.model)
            .ok_or_else(|| LocalError::UnknownModel(req.model.clone()))?;
        if !installed(&self.sidecar.config().models_dir, entry) {
            return Err(LocalError::NotInstalled(entry.id.clone()));
        }
        Ok(entry.clone())
    }

    /// Prywatność: profil lokalny dopuszcza każdą sesję; jurysdykcja nie dotyczy (dane nie
    /// opuszczają maszyny), chyba że profil wskazuje konkretną jurysdykcję.
    fn privacy(&self, req: &ChatRequest) -> Result<(), ProviderError> {
        let profile = &self.sidecar.config().privacy;
        let allow = if profile.jurisdiction == "local" {
            Vec::new()
        } else {
            req.meta.privacy.jurisdiction_allow.clone()
        };
        let privacy = RequestPrivacy {
            tag: req.meta.privacy.tag,
            jurisdiction_allow: allow,
        };
        check_privacy(&privacy, profile)
    }
}

#[async_trait]
impl ModelProvider for LocalProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        let ctx = self.sidecar.config().ctx;
        let models: BTreeMap<String, _> = self
            .models
            .iter()
            .map(|m| (m.id.clone(), m.capabilities(ctx)))
            .collect();
        let default = &self.sidecar.config().default_model;
        let installed = self.installed();
        let default_model = installed
            .iter()
            .find(|m| &m.id == default)
            .or_else(|| installed.first())
            .map(|m| m.id.clone());
        ProviderCapabilities {
            provider: self.id.clone(),
            default_model,
            models,
            interruption: InterruptionRendering::AppendNote,
            privacy: self.sidecar.config().privacy.clone(),
        }
    }

    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        if cancel.is_cancelled() {
            return single(ProviderEvent::stop(StopReason::Cancelled));
        }
        if let Err(e) = request.validate().and_then(|()| self.privacy(&request)) {
            return single(ProviderEvent::Error(e));
        }
        let entry = match self.admit(&request) {
            Ok(e) => e,
            Err(e) => return single(ProviderEvent::Error(e.to_provider_error())),
        };
        let sidecar = Arc::clone(&self.sidecar);
        sidecar.begin();
        let busy = Busy(Arc::clone(&sidecar));
        let path = self.model_path(&entry);
        let started = ProviderEvent::Started {
            model: entry.id.clone(),
            response_id: None,
        };
        let rest = futures_util::stream::once(async move {
            let engine = tokio::select! {
                biased;
                () = cancel.cancelled() => Err(LocalError::Cancelled),
                r = sidecar.ensure_running(&entry, &path) => r,
            };
            match engine {
                Err(LocalError::Cancelled) => single(ProviderEvent::stop(StopReason::Cancelled)),
                Err(e) => single(ProviderEvent::Error(e.to_provider_error())),
                Ok(engine) => {
                    let mut req = request;
                    req.meta.privacy.jurisdiction_allow.clear();
                    let inner = engine.stream(req, cancel);
                    Box::pin(inner.filter(|e| {
                        std::future::ready(!matches!(e, ProviderEvent::Started { .. }))
                    })) as ProviderStream
                }
            }
        })
        .flatten();
        Box::pin(
            futures_util::stream::iter([started])
                .chain(rest)
                .map(move |e| {
                    if let ProviderEvent::Error(err) = &e
                        && matches!(
                            err.kind,
                            ProviderErrorKind::Network
                                | ProviderErrorKind::Protocol
                                | ProviderErrorKind::Timeout { .. }
                        )
                    {
                        busy.0.mark_suspect();
                    }
                    e
                }),
        )
    }

    /// `Unconfigured` bez pobranego modelu (Router go pomija); inaczej zdrowy — awarie sidecara
    /// obsługuje restart, a trwałe problemy Router widzi jako błędy wywołań.
    fn health(&self) -> ProviderHealth {
        if self.installed().is_empty() {
            return ProviderHealth {
                state: HealthState::Unconfigured,
                ..ProviderHealth::healthy()
            };
        }
        ProviderHealth::healthy()
    }

    fn estimate_cost(&self, request: &ChatRequest) -> Option<CostEstimate> {
        let entry = self.entry(&request.model)?;
        let max_out = request
            .params
            .max_tokens
            .unwrap_or(self.sidecar.config().ctx.min(entry.ctx) / 2);
        Some(FREE.estimate(request, u64::from(max_out)))
    }

    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost> {
        self.entry(model).map(|_| FREE.cost(usage))
    }

    /// Modele zainstalowane (bez ruchu sieciowego).
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let ctx = self.sidecar.config().ctx;
        Ok(self
            .installed()
            .into_iter()
            .map(|m| ModelInfo {
                id: m.id.clone(),
                display_name: Some(m.name.clone()),
                created: None,
                capabilities: Some(m.capabilities(ctx)),
            })
            .collect())
    }
}
