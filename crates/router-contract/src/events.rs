//! Zdarzenia `router.*` — wyłącznie metadane decyzji (bez treści rozmowy).

use core_bus_contract::EventKind;
use providers_contract::{ProviderErrorKind, ProviderId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::TaskClass;
use crate::candidate::Candidate;
use crate::decision::{RejectReason, RouteDecision};

/// Decyzja trasy (klasa, wybrany cel, fallbacki, odrzuceni z powodem).
pub const EVENT_DECISION: &str = "router.decision";
/// Przełączenie na kolejnego kandydata.
pub const EVENT_FALLBACK: &str = "router.fallback";
/// Obwód dostawcy otwarty.
pub const EVENT_BREAKER_OPENED: &str = "router.breaker.opened";
/// Obwód dostawcy zamknięty.
pub const EVENT_BREAKER_CLOSED: &str = "router.breaker.closed";
/// Brak trasy (UI: czego brakuje — np. „dodaj klucz").
pub const EVENT_NO_ROUTE: &str = "router.no_route";
/// Okno limitu dostawcy wyczerpane (429).
pub const EVENT_PLAN_WINDOW_EXHAUSTED: &str = "router.plan_window.exhausted";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Powód przełączenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "cause", rename_all = "snake_case")]
pub enum FallbackCause {
    /// Błąd dostawcy przed pierwszym tokenem.
    Error {
        /// Rodzaj.
        kind: ProviderErrorKind,
    },
    /// Brak pierwszego zdarzenia w terminie klasy.
    Deadline {
        /// Termin (ms).
        deadline_ms: u64,
    },
}

/// Zdarzenie Routera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RouterEvent {
    /// `router.decision`.
    Decision {
        /// Decyzja.
        decision: RouteDecision,
    },
    /// `router.fallback`.
    Fallback {
        /// Klasa.
        class: TaskClass,
        /// Porzucony cel.
        from: Candidate,
        /// Następny cel.
        to: Candidate,
        /// Powód.
        cause: FallbackCause,
        /// Czas od początku żądania do przełączenia (ms).
        elapsed_ms: u64,
    },
    /// `router.breaker.opened`.
    BreakerOpened {
        /// Dostawca.
        provider: ProviderId,
        /// Do kiedy (ms zegara Routera).
        until_ms: u64,
    },
    /// `router.breaker.closed`.
    BreakerClosed {
        /// Dostawca.
        provider: ProviderId,
    },
    /// `router.no_route`.
    NoRoute {
        /// Klasa.
        class: TaskClass,
        /// Odrzuceni z powodem.
        rejected: Vec<(Candidate, RejectReason)>,
    },
    /// `router.plan_window.exhausted`.
    PlanWindowExhausted {
        /// Dostawca.
        provider: ProviderId,
        /// Za ile ms okno się odnowi (estymata).
        retry_in_ms: u64,
        /// Czy estymata pochodzi z nagłówka `retry-after`.
        from_header: bool,
    },
}

impl RouterEvent {
    /// Nazwa zdarzenia na magistrali.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Decision { .. } => EVENT_DECISION,
            Self::Fallback { .. } => EVENT_FALLBACK,
            Self::BreakerOpened { .. } => EVENT_BREAKER_OPENED,
            Self::BreakerClosed { .. } => EVENT_BREAKER_CLOSED,
            Self::NoRoute { .. } => EVENT_NO_ROUTE,
            Self::PlanWindowExhausted { .. } => EVENT_PLAN_WINDOW_EXHAUSTED,
        }
    }
}
