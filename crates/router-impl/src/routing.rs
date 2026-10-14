//! `RouterCore` — implementacja `Router`: rejestr dostawców, polityka (automatyczna albo jawna),
//! obwody i okna limitów per dostawca, zdarzenia `router.*` w kolejności.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use compliance_contract::Compliance;
use providers_contract::{ChatRequest, HealthState, ModelProvider, ProviderErrorKind, ProviderId};
use router_contract::{
    BreakerState, BreakerTransition, BudgetGate, Candidate, CircuitBreaker, Constraints,
    DIFFICULTY_CLASSES, DifficultyPolicy, Outcome, PlanWindow, RouteDecision, RouteError,
    RouteKind, RoutePolicy, Router, RouterClock, RouterEvent, TaskClass,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::clock::TokioClock;
use crate::evaluate::{Checked, Inputs, check};
use crate::helpers::counts_as_failure;

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

/// Kolejki modeli: dostawca → klasa zadań → modele w kolejności prób.
pub type ProviderQueues = BTreeMap<ProviderId, BTreeMap<TaskClass, Vec<String>>>;

/// Wspólna kolejka wszystkich kont: klasa zadań → kandydaci (dostawca i model) w kolejności prób.
pub type GlobalQueues = BTreeMap<TaskClass, Vec<Candidate>>;

/// Klasy zadań, które bierze lekki dostawca lokalny: prace w tle, którym wystarcza mały model
/// i które nie powinny zajmować karty głównej (streszczenia, tytuły, porządki pamięci).
pub const LIGHT_CLASSES: [TaskClass; 1] = [TaskClass::Summarize];

/// Router.
pub struct RouterCore {
    registry: RwLock<Vec<(ProviderId, Registered)>>,
    explicit: RwLock<Option<RoutePolicy>>,
    overrides: RwLock<Option<String>>,
    /// Kolejki modeli dostawców API: dostawca → klasa → modele w kolejności prób.
    queues: RwLock<ProviderQueues>,
    /// Wspólna kolejka wszystkich kont — pierwszeństwo przed kolejkami kont i modelem lekkim.
    global: RwLock<GlobalQueues>,
    /// Lekki dostawca lokalny (model pomocniczy na karcie zintegrowanej) — pierwszy w klasach
    /// [`LIGHT_CLASSES`], poza nimi nieużywany.
    light: RwLock<Option<ProviderId>>,
    /// Dobór według trudności (Ustawienia → Dobór modelu).
    pub(crate) difficulty: RwLock<DifficultyPolicy>,
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
            queues: RwLock::new(ProviderQueues::new()),
            global: RwLock::new(GlobalQueues::new()),
            light: RwLock::new(None),
            difficulty: RwLock::new(DifficultyPolicy::default()),
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

    /// Zeruje pamięć o kłopotach dostawcy (bezpiecznik, okno limitu zapytań). Wołane, gdy pod
    /// tym samym identyfikatorem zaczyna odpowiadać inne konto — jego limity są osobne.
    pub fn reset_health(&self, id: &ProviderId) {
        let mut health = lock(&self.health);
        health.breakers.remove(id);
        health.windows.remove(id);
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

    /// Kolejki modeli dostawców (z kont użytkownika). W polityce automatycznej dostawca
    /// z kolejką dla danej klasy wystawia jej modele po kolei zamiast jednego modelu domyślnego.
    pub fn set_provider_queues(&self, queues: ProviderQueues) {
        *self.queues.write().unwrap_or_else(|p| p.into_inner()) = queues;
    }

    /// Wspólna kolejka wszystkich kont (Ustawienia → Kolejki modeli API → „Wszystkie konta”).
    /// Dla klasy z niepustą kolejką jej kandydaci idą pierwsi, a reszta polityki zostaje za nimi
    /// jako zapas.
    pub fn set_global_queues(&self, queues: GlobalQueues) {
        *self.global.write().unwrap_or_else(|p| p.into_inner()) = queues;
    }

    /// Wskazuje lekkiego dostawcę lokalnego (zarejestrowanego jak każdy inny, rodzaj `Local`).
    /// W klasach [`LIGHT_CLASSES`] jest próbowany pierwszy; w pozostałych nie występuje, a rolę
    /// modelu lokalnego pełni dostawca główny.
    pub fn set_light_local(&self, id: Option<ProviderId>) {
        *self.light.write().unwrap_or_else(|p| p.into_inner()) = id;
    }

    /// Lekki dostawca lokalny (jeśli wskazany).
    pub(crate) fn light_id(&self) -> Option<ProviderId> {
        self.light.read().unwrap_or_else(|p| p.into_inner()).clone()
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
        let light_id = self.light_id();
        let local = reg
            .iter()
            .filter(|(id, r)| r.kind == RouteKind::Local && Some(id) != light_id.as_ref())
            .find_map(|(_, r)| default(r));
        let light = reg
            .iter()
            .filter(|(id, r)| r.kind == RouteKind::Local && Some(id) == light_id.as_ref())
            .find_map(|(_, r)| default(r));
        let api: Vec<Candidate> = reg
            .iter()
            .filter(|(_, r)| {
                r.kind == RouteKind::Api && r.provider.health().state != HealthState::Unconfigured
            })
            .filter_map(|(_, r)| default(r))
            .collect();
        let mut policy = RoutePolicy::defaults(local.as_ref(), &api);
        let queues = self.queues.read().unwrap_or_else(|p| p.into_inner());
        crate::helpers::apply_queues(&mut policy.classes, &queues);
        if let Some(light) = light {
            for class in LIGHT_CLASSES {
                policy
                    .classes
                    .entry(class)
                    .or_default()
                    .insert(0, light.clone());
            }
            // Dobór według trudności: lekki model lokalny jako szczebel „lekki” zwykłej pracy.
            if self.difficulty_policy().enabled {
                for class in DIFFICULTY_CLASSES {
                    let list = policy.classes.entry(class).or_default();
                    if !list.contains(&light) {
                        list.push(light.clone());
                    }
                }
            }
        }
        let global = self.global.read().unwrap_or_else(|p| p.into_inner());
        crate::helpers::put_first(&mut policy.classes, &global);
        policy
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
        let pinned: Vec<Candidate> = constraints.pinned.iter().cloned().collect();
        let mut rest: Vec<Candidate> = Vec::new();
        for c in policy.candidates(class) {
            if !pinned.contains(c) && !rest.contains(c) {
                rest.push(c.clone());
            }
        }
        // Przypięty model zostaje pierwszy; reszta według trudności zadania.
        let difficulty = self.judge(class, constraints, request);
        if let Some(note) = &difficulty {
            rest = self.arrange(rest, note);
        }
        let list = pinned.into_iter().chain(rest);
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
            difficulty,
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
