//! Router (docs/modules/router/SPEC.md, PLAN §5.4, §6.9, ADR 0005, ADR 0014).
//!
//! - [`RouterCore`] — `Router`: klasa × ograniczenia → decyzja z uzasadnieniem (rejestr zgodności
//!   `route_allowed` + jurysdykcja, `check_privacy` w głąb, możliwości modelu, obwód, okno limitu
//!   429, TTFT, budżet `cost-meter`), polityka automatyczna (bez kluczy → wszystko lokalnie;
//!   z kluczem → rozmowa przez API, głos-szybka lokalnie) albo jawna/TOML, obwody per dostawca;
//! - [`RoutedProvider`] — Router jako `ModelProvider` (dekorator): fallback przed pierwszą treścią
//!   ≤ 2 s bez utraty wiadomości, błąd po częściowym wyjściu nie jest maskowany, pochodzenie bloków
//!   myślenia zachowane (`dostawca:model`);
//! - [`CostMeterGate`] — `BudgetGate` przez `cost_meter_contract::evaluate`;
//! - [`RouterModule`] — moduł rejestru; zdarzenia `router.*` na magistralę (bez treści).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod clock;
mod difficulty;
mod evaluate;
mod expert_local;
mod fallback;
mod gate;
mod helpers;
mod history;
mod main_local;
mod module;
mod routed;
mod router_trait;
mod routing;

pub use clock::{ManualClock, TokioClock};
pub use gate::CostMeterGate;
pub use helpers::{counts_as_failure, reason_code, router_with, warning_code};
pub use history::{localize_history, qualify_event};
pub use module::{MODULE_TOML, RouterModule};
pub use routed::RoutedProvider;
pub use routing::{GlobalQueues, LIGHT_CLASSES, ProviderQueues, Registered, RouterCore};
