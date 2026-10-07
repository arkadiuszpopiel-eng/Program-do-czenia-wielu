//! `RoutedProvider` — Router jako `ModelProvider` (dekorator): reszta systemu (agent-runtime,
//! voice-dialog) woła zwykły `stream`, a Router wybiera cel wg klasy i ograniczeń żądania,
//! nadzoruje fallback i publikuje `router.decision`.
//!
//! Modele Routera to `dostawca:model`; `ChatRequest::model = "auto"` (albo dowolny
//! niekwalifikowany) = wybór wg klasy, `dostawca:model` = przypięcie (próbowane pierwsze).

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, EmbeddingRequest, EmbeddingResponse,
    HealthState, InterruptionRendering, ModelInfo, ModelProvider, ProviderCapabilities,
    ProviderError, ProviderErrorKind, ProviderEvent, ProviderHealth, ProviderId, ProviderPrivacy,
    ProviderStream, StopReason, Usage,
};
use router_contract::{
    AUTO_MODEL, Candidate, Constraints, Outcome, ROUTER_PROVIDER_ID, RouteDecision, RouteError,
    Router, RouterEvent, TaskClass,
};
use tokio::sync::mpsc;

use crate::fallback::Run;
use crate::routing::RouterCore;

fn single(event: ProviderEvent) -> ProviderStream {
    Box::pin(futures_util::stream::iter([event]))
}

/// Router jako dostawca dla jednej klasy zadań.
pub struct RoutedProvider {
    core: Arc<RouterCore>,
    id: ProviderId,
    class: TaskClass,
    background: bool,
    max_latency_ms: Option<u64>,
}

impl RoutedProvider {
    /// Dostawca klasy (id `router`).
    pub fn new(core: Arc<RouterCore>, class: TaskClass) -> Self {
        Self {
            core,
            id: ProviderId::new(ROUTER_PROVIDER_ID),
            class,
            background: false,
            max_latency_ms: None,
        }
    }

    /// Zadania tła (budżet tła w `cost-meter`).
    #[must_use]
    pub fn background(mut self, background: bool) -> Self {
        self.background = background;
        self
    }

    /// Limit czasu do pierwszego tokenu wg ostatnich pomiarów (np. ścieżka głosu).
    #[must_use]
    pub fn max_latency_ms(mut self, ms: Option<u64>) -> Self {
        self.max_latency_ms = ms;
        self
    }

    /// Klasa.
    pub fn class(&self) -> TaskClass {
        self.class
    }

    fn constraints(&self, req: &ChatRequest) -> Constraints {
        let mut c = Constraints::from_request(self.class, req);
        c.background = self.background;
        c.max_latency_ms = self.max_latency_ms;
        c
    }

    fn provider_of(&self, c: &Candidate) -> Option<Arc<dyn ModelProvider>> {
        self.core.registered(&c.provider).map(|r| r.provider)
    }

    fn decide_quiet(&self, req: &ChatRequest) -> Result<RouteDecision, RouteError> {
        self.core
            .decide(self.class, &self.constraints(req), Some(req))
    }
}

#[async_trait]
impl ModelProvider for RoutedProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    /// Modele wszystkich zarejestrowanych dostawców jako `dostawca:model`.
    fn capabilities(&self) -> ProviderCapabilities {
        let mut models = BTreeMap::new();
        for (id, reg) in self.core.all_registered() {
            for (model, caps) in reg.provider.capabilities().models {
                models.insert(Candidate::new(id.clone(), model).qualified(), caps);
            }
        }
        ProviderCapabilities {
            provider: self.id.clone(),
            default_model: Some(AUTO_MODEL.to_owned()),
            models,
            interruption: InterruptionRendering::AppendNote,
            // Prywatność egzekwuje wybór trasy; profil Routera jest tylko informacyjny.
            privacy: ProviderPrivacy::new("router", "unknown"),
        }
    }

    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        if cancel.is_cancelled() {
            return single(ProviderEvent::stop(StopReason::Cancelled));
        }
        let mut probe = request.clone();
        if Candidate::parse(&probe.model).is_none() {
            // „auto" lub model niekwalifikowany — walidacja nie może odrzucić pustego modelu.
            probe.model = AUTO_MODEL.to_owned();
        }
        if let Err(e) = probe.validate() {
            return single(ProviderEvent::Error(e));
        }
        let constraints = self.constraints(&request);
        let decision = match self.core.route(self.class, &constraints, Some(&request)) {
            Ok(d) => d,
            Err(e) => return single(ProviderEvent::Error(e.to_provider_error())),
        };
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return single(ProviderEvent::Error(ProviderError::new(
                ProviderErrorKind::Protocol,
                "router wymaga środowiska tokio",
            )));
        };
        let (tx, rx) = mpsc::channel(64);
        let run = Run {
            core: Arc::clone(&self.core),
            router_id: self.id.clone(),
            class: self.class,
            decision,
            request,
            cancel,
            tx,
        };
        runtime.spawn(run.execute());
        Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
    }

    /// Zdrowie klasy: `Unconfigured` bez dostawców, `Unavailable` bez trasy, inaczej zdrowie celu.
    fn health(&self) -> ProviderHealth {
        if self.core.all_registered().is_empty() {
            return ProviderHealth {
                state: HealthState::Unconfigured,
                ..ProviderHealth::healthy()
            };
        }
        match self.core.decide(self.class, &Constraints::default(), None) {
            Ok(d) => self
                .provider_of(&d.chosen)
                .map_or_else(ProviderHealth::healthy, |p| p.health()),
            Err(_) => ProviderHealth {
                state: HealthState::Unavailable,
                ..ProviderHealth::healthy()
            },
        }
    }

    fn estimate_cost(&self, request: &ChatRequest) -> Option<CostEstimate> {
        let d = self.decide_quiet(request).ok()?;
        let mut req = request.clone();
        req.model.clone_from(&d.chosen.model);
        self.provider_of(&d.chosen)?.estimate_cost(&req)
    }

    /// Koszt dla modelu `dostawca:model` (jak w `Started` zwracanym przez Router).
    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost> {
        let c = Candidate::parse(model)?;
        self.provider_of(&c)?.cost(&c.model, usage)
    }

    /// Modele z możliwości zarejestrowanych dostawców (bez ruchu sieciowego).
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(self
            .capabilities()
            .models
            .into_iter()
            .map(|(id, caps)| ModelInfo {
                id,
                display_name: None,
                created: None,
                capabilities: Some(caps),
            })
            .collect())
    }

    /// Osadzenia z fallbackiem na kolejne cele klasy `Embeddings`.
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        let constraints = Constraints {
            needs: router_contract::CapabilityNeeds::for_class(TaskClass::Embeddings),
            pinned: Candidate::parse(&request.model),
            background: self.background,
            ..Constraints::default()
        };
        let decision = self
            .core
            .route(TaskClass::Embeddings, &constraints, None)
            .map_err(|e| e.to_provider_error())?;
        let targets: Vec<Candidate> = decision.targets().cloned().collect();
        let mut last = None;
        for (i, cand) in targets.iter().enumerate() {
            let Some(provider) = self.provider_of(cand) else {
                continue;
            };
            self.core.begin_attempt(cand);
            let t0 = tokio::time::Instant::now();
            let sub = EmbeddingRequest {
                model: cand.model.clone(),
                input: request.input.clone(),
            };
            match provider.embed(sub).await {
                Ok(out) => {
                    let latency_ms = u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX);
                    self.core.report(
                        cand,
                        Outcome::Ok {
                            ttft_ms: None,
                            latency_ms,
                        },
                    );
                    return Ok(out);
                }
                Err(e) => {
                    self.core.report(
                        cand,
                        Outcome::Failed {
                            kind: e.kind.clone(),
                        },
                    );
                    if let Some(to) = targets.get(i + 1).filter(|_| e.should_fallback()) {
                        self.core.emit(RouterEvent::Fallback {
                            class: TaskClass::Embeddings,
                            from: cand.clone(),
                            to: to.clone(),
                            cause: router_contract::FallbackCause::Error {
                                kind: e.kind.clone(),
                            },
                            elapsed_ms: u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX),
                        });
                        last = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        Err(last.unwrap_or_else(|| {
            ProviderError::new(ProviderErrorKind::Unsupported, "router: brak celu osadzeń")
        }))
    }
}
