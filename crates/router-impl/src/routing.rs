//! `RouterCore` — implementacja `Router`: rejestr dostawców, polityka (automatyczna albo jawna),
//! obwody i okna limitów per dostawca, zdarzenia `router.*` w kolejności.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use compliance_contract::Compliance;
use providers_contract::{ChatRequest, HealthState, ModelProvider, ProviderErrorKind, ProviderId};
use router_contract::{
    BreakerState, BreakerTransition, BudgetGate, Candidate, CircuitBreaker, Constraints, Outcome,
    PlanWindow, RejectReason, RouteDecision, RouteError, RouteKind, RoutePolicy, RouteWarning,
    Router, RouterClock, RouterEvent, TaskClass,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::clock::TokioClock;
use crate::evaluate::{Checked, Inputs, check};

/// Zarejestrowany dostawca.
#[derive(Clone)]
pub struct Registered {
    /// Dostawca.
    pub provider: Arc<dyn ModelProvider>,
    /// Rodzaj trasy.
    pub kind: RouteKind,
}

#[derive(Default)]
struct Health {
    breakers: BTreeMap<ProviderId, CircuitBreaker>,
    windows: BTreeMap<ProviderId, PlanWindow>,
}

/// Router.
pub struct RouterCore {
    registry: RwLock<Vec<(ProviderId, Registered)>>,
    explicit: RwLock<Option<RoutePolicy>>,
    overrides: RwLock<Option<String>>,
    compliance: Option<Arc<dyn Compliance>>,
    budget: Option<Arc<dyn BudgetGate>>,
    clock: Arc<dyn RouterClock>,
    health: Mutex<Health>,
    sink: Mutex<Option<UnboundedSender<RouterEvent>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Default for RouterCore {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl RouterCore {
    /// Router z rejestrem zgodności i bramką budżetu (brak = sprawdzenie pominięte).
    pub fn new(
        compliance: Option<Arc<dyn Compliance>>,
        budget: Option<Arc<dyn BudgetGate>>,
    ) -> Self {
        Self {
            registry: RwLock::new(Vec::new()),
            explicit: RwLock::new(None),
            overrides: RwLock::new(None),
            compliance,
            budget,
            clock: Arc::new(TokioClock::new()),
            health: Mutex::new(Health::default()),
            sink: Mutex::new(None),
        }
    }

    /// Zegar obwodów (testy: ręczny).
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn RouterClock>) -> Self {
        self.clock = clock;
        self
    }

    /// Rejestruje (albo zastępuje) dostawcę — bez restartu (nowy klucz = nowa trasa).
    pub fn register(&self, provider: Arc<dyn ModelProvider>, kind: RouteKind) {
        let id = provider.id().clone();
        let mut reg = self.registry.write().unwrap_or_else(|p| p.into_inner());
        reg.retain(|(p, _)| *p != id);
        reg.push((id, Registered { provider, kind }));
    }

    /// Wyrejestrowuje dostawcę.
    pub fn unregister(&self, id: &ProviderId) {
        self.registry
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|(p, _)| p != id);
    }

    /// Zarejestrowany dostawca.
    pub fn registered(&self, id: &ProviderId) -> Option<Registered> {
        self.registry
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .find(|(p, _)| p == id)
            .map(|(_, r)| r.clone())
    }

    /// Wszyscy zarejestrowani (w kolejności rejestracji).
    pub fn all_registered(&self) -> Vec<(ProviderId, Registered)> {
        self.registry
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Ustawia jawną politykę (zamiast automatycznej).
    pub fn set_policy(&self, policy: Option<RoutePolicy>) {
        *self.explicit.write().unwrap_or_else(|p| p.into_inner()) = policy;
    }

    /// Nadpisania `[router]` z konfiguracji nakładane na politykę automatyczną.
    pub fn set_overrides_toml(&self, text: Option<String>) -> Result<(), String> {
        if let Some(t) = &text {
            self.auto_policy().with_toml(t)?;
        }
        *self.overrides.write().unwrap_or_else(|p| p.into_inner()) = text;
        Ok(())
    }

    /// Polityka automatyczna z zarejestrowanych dostawców: lokalny (pierwszy `Local` z modelem
    /// domyślnym) + API skonfigurowane (z kluczem), w kolejności rejestracji.
    pub fn auto_policy(&self) -> RoutePolicy {
        let reg = self.all_registered();
        let default = |r: &Registered| {
            r.provider
                .capabilities()
                .default_model
                .map(|m| Candidate::new(r.provider.id().clone(), m))
        };
        let local = reg
            .iter()
            .filter(|(_, r)| r.kind == RouteKind::Local)
            .find_map(|(_, r)| default(r));
        let api: Vec<Candidate> = reg
            .iter()
            .filter(|(_, r)| {
                r.kind == RouteKind::Api && r.provider.health().state != HealthState::Unconfigured
            })
            .filter_map(|(_, r)| default(r))
            .collect();
        RoutePolicy::defaults(local.as_ref(), &api)
    }

    /// Podłącza kolejkę zdarzeń (moduł przekazuje je na magistralę).
    pub fn set_event_sink(&self, sink: Option<UnboundedSender<RouterEvent>>) {
        *lock(&self.sink) = sink;
    }

    pub(crate) fn emit(&self, event: RouterEvent) {
        if let Some(tx) = lock(&self.sink).as_ref() {
            // Moduł zatrzymany — zdarzenia diagnostyczne można pominąć.
            let _ = tx.send(event);
        }
    }

    /// Chwila zegara Routera (ms).
    pub fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    /// Decyzja bez publikacji zdarzeń (oszacowania, zdrowie).
    pub fn decide(
        &self,
        class: TaskClass,
        constraints: &Constraints,
        request: Option<&ChatRequest>,
    ) -> Result<RouteDecision, RouteError> {
        let policy = self.policy();
        let mut list: Vec<Candidate> = constraints.pinned.iter().cloned().collect();
        for c in policy.candidates(class) {
            if !list.contains(c) {
                list.push(c.clone());
            }
        }
        let now = self.clock.now_ms();
        // Migawka stanu obwodów — bez trzymania blokady podczas wywołań dostawców.
        let (breakers, windows) = {
            let h = lock(&self.health);
            (h.breakers.clone(), h.windows.clone())
        };
        let inputs = Inputs {
            compliance: self.compliance.as_deref(),
            budget: self.budget.as_deref(),
            breakers: &breakers,
            windows: &windows,
            now_ms: now,
        };
        let mut allowed = Vec::new();
        let mut rejected = Vec::new();
        let mut warnings = Vec::new();
        for cand in list {
            let registered = self.registered(&cand.provider);
            match check(&inputs, registered.as_ref(), &cand, constraints, request) {
                Checked::Allowed(w) => {
                    warnings.extend(w.into_iter().map(|w| (cand.clone(), w)));
                    allowed.push(cand);
                }
                Checked::Rejected(reason) => rejected.push((cand, reason)),
            }
        }
        let mut targets = allowed.into_iter();
        let Some(chosen) = targets.next() else {
            return Err(RouteError::NoRoute { class, rejected });
        };
        Ok(RouteDecision {
            class,
            chosen,
            fallbacks: targets.collect(),
            rejected,
            warnings,
        })
    }

    /// Początek próby kandydata (zajmuje próbę half-open).
    pub fn begin_attempt(&self, candidate: &Candidate) {
        let now = self.clock.now_ms();
        let config = self.policy().breaker;
        let mut h = lock(&self.health);
        let b = h
            .breakers
            .entry(candidate.provider.clone())
            .or_insert_with(|| CircuitBreaker::new(config));
        if b.begin_attempt(now).is_some() {
            tracing::info!(provider = %candidate.provider, "obwód: próba half-open");
        }
    }

    fn breaker_event(&self, provider: &ProviderId, t: Option<BreakerTransition>) {
        match t {
            Some(BreakerTransition::Opened { until_ms }) => {
                tracing::warn!(%provider, until_ms, "obwód otwarty");
                self.emit(RouterEvent::BreakerOpened {
                    provider: provider.clone(),
                    until_ms,
                });
            }
            Some(BreakerTransition::Closed) => {
                self.emit(RouterEvent::BreakerClosed {
                    provider: provider.clone(),
                });
            }
            Some(BreakerTransition::HalfOpened) | None => {}
        }
    }
}

impl Router for RouterCore {
    fn route(
        &self,
        class: TaskClass,
        constraints: &Constraints,
        request: Option<&ChatRequest>,
    ) -> Result<RouteDecision, RouteError> {
        let result = self.decide(class, constraints, request);
        match &result {
            Ok(decision) => self.emit(RouterEvent::Decision {
                decision: decision.clone(),
            }),
            Err(RouteError::NoRoute { class, rejected }) => {
                tracing::warn!(?class, rejected = rejected.len(), "brak trasy");
                self.emit(RouterEvent::NoRoute {
                    class: *class,
                    rejected: rejected.clone(),
                });
            }
        }
        result
    }

    fn report(&self, candidate: &Candidate, outcome: Outcome) {
        let now = self.clock.now_ms();
        let config = self.policy().breaker;
        let provider = &candidate.provider;
        let (transition, window) = {
            let mut h = lock(&self.health);
            let window = match &outcome {
                Outcome::Failed {
                    kind: ProviderErrorKind::RateLimited { retry_after_ms },
                } => {
                    let until = h
                        .windows
                        .entry(provider.clone())
                        .or_default()
                        .on_rate_limited(now, *retry_after_ms);
                    Some((until.saturating_sub(now), retry_after_ms.is_some()))
                }
                Outcome::Ok { .. } => {
                    h.windows.remove(provider);
                    None
                }
                _ => None,
            };
            let b = h
                .breakers
                .entry(provider.clone())
                .or_insert_with(|| CircuitBreaker::new(config));
            let transition = match &outcome {
                Outcome::Ok { .. } => b.on_success(),
                Outcome::Failed { kind } if counts_as_failure(kind) => b.on_failure(now),
                Outcome::Failed { .. } | Outcome::Cancelled => {
                    b.on_cancel();
                    None
                }
            };
            (transition, window)
        };
        self.breaker_event(provider, transition);
        if let Some((retry_in_ms, from_header)) = window {
            self.emit(RouterEvent::PlanWindowExhausted {
                provider: provider.clone(),
                retry_in_ms,
                from_header,
            });
        }
    }

    fn breaker_state(&self, provider: &ProviderId) -> BreakerState {
        let now = self.clock.now_ms();
        lock(&self.health)
            .breakers
            .get(provider)
            .map_or(BreakerState::Closed, |b| b.state(now))
    }

    fn policy(&self) -> RoutePolicy {
        if let Some(p) = self
            .explicit
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
        {
            return p;
        }
        let auto = self.auto_policy();
        let overrides = self
            .overrides
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        match overrides {
            // Nadpisania zwalidowane przy ustawieniu; błąd tu = polityka automatyczna.
            Some(t) => auto.clone().with_toml(&t).unwrap_or(auto),
            None => auto,
        }
    }
}

/// Czy błąd obciąża dostawcę (obwód): nie — odrzucenia żądania, prywatność, brak funkcji,
/// limit 429 (osobne okno).
pub fn counts_as_failure(kind: &ProviderErrorKind) -> bool {
    !matches!(
        kind,
        ProviderErrorKind::InvalidRequest
            | ProviderErrorKind::PrivacyBlocked
            | ProviderErrorKind::Unsupported
            | ProviderErrorKind::RateLimited { .. }
    )
}

/// Ułatwienie dla kompozycji: `Arc<RouterCore>` z dostawcami.
pub fn router_with(providers: Vec<(Arc<dyn ModelProvider>, RouteKind)>) -> Arc<RouterCore> {
    let core = RouterCore::default();
    for (p, kind) in providers {
        core.register(p, kind);
    }
    Arc::new(core)
}

/// Pojedyncze odrzucenie — dla testów i UI.
pub fn reason_code(reason: &RejectReason) -> String {
    serde_json::to_value(reason)
        .ok()
        .and_then(|v| v["code"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Ostrzeżenie — kod.
pub fn warning_code(warning: &RouteWarning) -> String {
    serde_json::to_value(warning)
        .ok()
        .and_then(|v| v["code"].as_str().map(str::to_owned))
        .unwrap_or_default()
}
