//! Atrapa Routera (docs/modules/router/SPEC.md §Fake): skryptowane decyzje per klasa, domyślnie
//! „pierwszy kandydat polityki z zamkniętym obwodem", zapis `report()`, obwody z kontraktu na
//! ręcznym zegarze — dla testów `agent-runtime`, `voice-dialog`.
//!
//! Tylko jako `dev-dependency` innych modułów (crates/README.md).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use providers_contract::{ChatRequest, ProviderErrorKind, ProviderId};
use router_contract::{
    BreakerState, Candidate, CircuitBreaker, Constraints, Outcome, RejectReason, RouteDecision,
    RouteError, RoutePolicy, Router, TaskClass,
};

/// Zapisane wywołanie `route`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteCall {
    /// Klasa.
    pub class: TaskClass,
    /// Ograniczenia.
    pub constraints: Constraints,
    /// Model z żądania (jeśli podano żądanie).
    pub model: Option<String>,
}

struct State {
    policy: RoutePolicy,
    scripted: BTreeMap<TaskClass, VecDeque<Result<RouteDecision, RouteError>>>,
    calls: Vec<RouteCall>,
    reports: Vec<(Candidate, Outcome)>,
    breakers: BTreeMap<ProviderId, CircuitBreaker>,
}

/// Deterministyczny Router.
pub struct FakeRouter {
    state: Mutex<State>,
    now_ms: AtomicU64,
}

impl FakeRouter {
    /// Atrapa z polityką.
    pub fn new(policy: RoutePolicy) -> Self {
        Self {
            state: Mutex::new(State {
                policy,
                scripted: BTreeMap::new(),
                calls: Vec::new(),
                reports: Vec::new(),
                breakers: BTreeMap::new(),
            }),
            now_ms: AtomicU64::new(0),
        }
    }

    /// Następne `route` dla klasy zwróci ten wynik (kolejka).
    pub fn push_route(&self, class: TaskClass, result: Result<RouteDecision, RouteError>) {
        self.lock()
            .scripted
            .entry(class)
            .or_default()
            .push_back(result);
    }

    /// Wywołania `route`.
    pub fn calls(&self) -> Vec<RouteCall> {
        self.lock().calls.clone()
    }

    /// Zgłoszone wyniki.
    pub fn reports(&self) -> Vec<(Candidate, Outcome)> {
        self.lock().reports.clone()
    }

    /// Przesuwa zegar obwodów.
    pub fn advance_ms(&self, ms: u64) {
        self.now_ms.fetch_add(ms, Ordering::SeqCst);
    }

    fn now(&self) -> u64 {
        self.now_ms.load(Ordering::SeqCst)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl Router for FakeRouter {
    fn route(
        &self,
        class: TaskClass,
        constraints: &Constraints,
        request: Option<&ChatRequest>,
    ) -> Result<RouteDecision, RouteError> {
        let now = self.now();
        let mut st = self.lock();
        st.calls.push(RouteCall {
            class,
            constraints: constraints.clone(),
            model: request.map(|r| r.model.clone()),
        });
        if let Some(result) = st.scripted.get_mut(&class).and_then(VecDeque::pop_front) {
            return result;
        }
        let mut list: Vec<Candidate> = constraints.pinned.iter().cloned().collect();
        for c in st.policy.candidates(class) {
            if !list.contains(c) {
                list.push(c.clone());
            }
        }
        let mut allowed = Vec::new();
        let mut rejected = Vec::new();
        for c in list {
            match st.breakers.get(&c.provider).map(|b| b.admits(now)) {
                Some(Err(retry_in_ms)) => {
                    rejected.push((c, RejectReason::CircuitOpen { retry_in_ms }));
                }
                _ => allowed.push(c),
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
            warnings: Vec::new(),
        })
    }

    fn report(&self, candidate: &Candidate, outcome: Outcome) {
        let now = self.now();
        let mut st = self.lock();
        let config = st.policy.breaker;
        let breaker = st
            .breakers
            .entry(candidate.provider.clone())
            .or_insert_with(|| CircuitBreaker::new(config));
        match &outcome {
            Outcome::Ok { .. } => {
                breaker.on_success();
            }
            Outcome::Failed { kind } if *kind != ProviderErrorKind::InvalidRequest => {
                breaker.on_failure(now);
            }
            Outcome::Failed { .. } | Outcome::Cancelled => breaker.on_cancel(),
        }
        st.reports.push((candidate.clone(), outcome));
    }

    fn breaker_state(&self, provider: &ProviderId) -> BreakerState {
        let now = self.now();
        self.lock()
            .breakers
            .get(provider)
            .map_or(BreakerState::Closed, |b| b.state(now))
    }

    fn policy(&self) -> RoutePolicy {
        self.lock().policy.clone()
    }
}
