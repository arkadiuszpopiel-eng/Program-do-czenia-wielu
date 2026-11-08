//! `Router` dla `RouterCore`: decyzja z publikacją zdarzeń, przyjmowanie wyników wywołań
//! (obwody, okna limitów 429), stan obwodu i obowiązująca polityka (jawna albo automatyczna
//! z nadpisaniami `[router]`).

use providers_contract::{ChatRequest, ProviderErrorKind, ProviderId};
use router_contract::{
    BreakerState, Candidate, CircuitBreaker, Constraints, Outcome, RouteDecision, RouteError,
    RoutePolicy, Router, RouterEvent, TaskClass,
};

use crate::helpers::counts_as_failure;
use crate::routing::{RouterCore, lock};

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
