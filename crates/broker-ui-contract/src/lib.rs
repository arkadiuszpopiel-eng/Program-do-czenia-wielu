//! Kontrakt Broker-UI (docs/modules/broker-ui/SPEC.md, PLAN §8.2, ADR 3, THREAT_MODEL S11).
//!
//! Model prezentacji prośby ([`ApprovalCard`]: kto, co, zakres, ryzyko, odwracalność, plan,
//! „tylko teraz / zawsze w tym zakresie ≤ 24 h”, czas do wygaśnięcia), reguły przyjęcia wejścia
//! jako dowodu fizycznego ([`check_input`]: wejście niewstrzyknięte, okno na pierwszym planie
//! ≥ 500 ms, niezasłonięte) oraz traity [`BrokerUi`], [`BrokerLink`], [`HelloPort`].
//! `PhysicalInputProof` powstaje wyłącznie w `broker-ui-impl` (i w atrapie testowej).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod api;
mod card;
mod guard;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use api::{
    BrokerLink, BrokerUi, EVENT_DECIDED, EVENT_HELLO_USED, EVENT_INJECTION_REJECTED,
    EVENT_INPUT_REJECTED, EVENT_SHOWN, EVENT_WITHDRAWN, HelloOutcome, HelloPort, NoHello, UiConfig,
    UiDecision, UiError, UiEvent, UiStatus,
};
pub use card::{
    ApprovalCard, CardOptions, DecisionOption, GRANT_MAX_MS, MAX_PLAN_LINES, MAX_VALUE_CHARS,
    PersonaDisplay, PlanLine, persona_of, sanitize, time_left_pl,
};
pub use guard::{ForegroundTracker, MIN_FOREGROUND_MS, RejectReason, check_input, source_for};
