//! Kontrakt Routera (docs/modules/router/SPEC.md, PLAN §5.4, §6.9, ADR 0005, ADR 0014).
//!
//! Router kieruje **zadania**: klasa ([`TaskClass`]: głos-szybka, rozmowa, kod, planowanie,
//! GUI/wizja, streszczanie, embeddingi) × ograniczenia ([`Constraints`]: tag prywatności
//! i jurysdykcji sesji — `compliance-contract::route_allowed`; budżet — `cost-meter-contract::evaluate`
//! przez [`BudgetGate`]; możliwości z `ModelCapabilities`; opóźnienie) → [`RouteDecision`]
//! z uzasadnieniem (`chosen`, `fallbacks`, `rejected: Vec<(kandydat, powód)>`), publikowane jako
//! `router.decision` (bez treści). Tabela tras [`RoutePolicy`] jest konfigurowalna
//! (`[router.class.<klasa>] prefer = ["dostawca:model", …]`); domyślnie bez kluczy wszystko
//! lokalnie (profil A).
//!
//! Czysta logika wspólna dla `-impl` i `-fake`: [`CircuitBreaker`] (N błędów w oknie → otwarty na T,
//! próba half-open), [`PlanWindow`] (reaktywna estymata okien z 429 + `retry-after`),
//! [`CapabilityNeeds::check`]. Mówczyni + Myślicielka (§6.9) — tylko [`DuoConfig`]/[`Tempo`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod breaker;
mod candidate;
#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod decision;
mod events;
mod needs;
mod policy;

pub use accounts_hub_contract::TaskClass;
pub use breaker::{
    BreakerConfig, BreakerState, BreakerTransition, CircuitBreaker, PLAN_WINDOW_BASE_MS,
    PLAN_WINDOW_MAX_MS, PlanWindow,
};
pub use candidate::{Candidate, RouteKind};
pub use decision::{Outcome, RejectReason, RouteDecision, RouteError, RouteWarning};
pub use events::{
    EVENT_BREAKER_CLOSED, EVENT_BREAKER_OPENED, EVENT_DECISION, EVENT_FALLBACK, EVENT_NO_ROUTE,
    EVENT_PLAN_WINDOW_EXHAUSTED, FallbackCause, RouterEvent, event_kind,
};
pub use needs::{CapabilityNeeds, Constraints, MissingCapability};
pub use policy::{ALL_CLASSES, DuoConfig, RoutePolicy, Tempo, default_deadlines, parse_duration};

use cost_meter_contract::BudgetDecision;
use providers_contract::{ChatRequest, ProviderId};

/// Identyfikator Routera jako dostawcy (`ModelProvider::id`); modele Routera to `dostawca:model`.
pub const ROUTER_PROVIDER_ID: &str = "router";

/// Model „wybierz wg klasy" (bez przypięcia) w `ChatRequest::model` kierowanym do Routera.
pub const AUTO_MODEL: &str = "auto";

/// Bramka budżetu: decyzja `cost-meter` dla szacowanego kosztu (mikro-USD) u dostawcy.
/// Produkcyjnie: `cost_meter_contract::evaluate` na bieżących wydatkach i kursie.
pub trait BudgetGate: Send + Sync {
    /// `Allow` / `Warn` / `Block` dla zadania.
    fn check(
        &self,
        provider: &ProviderId,
        estimate_micro_usd: u64,
        background: bool,
    ) -> BudgetDecision;
}

/// Monotoniczny zegar Routera w milisekundach (obwody, okna limitów).
pub trait RouterClock: Send + Sync {
    /// Milisekundy od stałego punktu.
    fn now_ms(&self) -> u64;
}

/// Router: deterministyczny wybór trasy (ta sama polityka + stan = ta sama decyzja; bez I/O,
/// ≤ 1 ms) i przyjmowanie wyników wywołań (obwody, okna limitów).
pub trait Router: Send + Sync {
    /// Decyzja dla klasy i ograniczeń; `request` służy do oszacowania kosztu (budżet).
    fn route(
        &self,
        class: TaskClass,
        constraints: &Constraints,
        request: Option<&ChatRequest>,
    ) -> Result<RouteDecision, RouteError>;

    /// Wynik wywołania kandydata.
    fn report(&self, candidate: &Candidate, outcome: Outcome);

    /// Stan obwodu dostawcy.
    fn breaker_state(&self, provider: &ProviderId) -> BreakerState;

    /// Obowiązująca polityka (po uwzględnieniu zarejestrowanych dostawców).
    fn policy(&self) -> RoutePolicy;
}

/// JSON Schema decyzji (UI: „dlaczego ten model").
pub fn decision_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(RouteDecision)).unwrap_or_default()
}

/// JSON Schema zdarzeń `router.*`.
pub fn event_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(RouterEvent)).unwrap_or_default()
}

#[cfg(test)]
mod tests;
