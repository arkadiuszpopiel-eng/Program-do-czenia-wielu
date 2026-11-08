//! Kontrakt modułu `scheduler-lite` (docs/modules/scheduler-lite/SPEC.md, PLAN §9.3–9.4, §6.5).
//!
//! Deterministyczny menedżer **zasobów wyłącznych** (głośnik/mówienie, mikrofon, ekran+wejście,
//! wskazane pliki): kolejka priorytetowa, dzierżawy RAII ([`Lease`]), wywłaszczanie „voice-first”
//! tylko w punktach atomowych (sygnał, nie zabijanie), timeouty z `on_timeout: ask_user|fail`,
//! kolejka mówienia z przekazaniem bez luki, wykrywanie zakleszczeń (graf oczekiwania).
//!
//! Crate zawiera trait [`SchedulerLite`], typy, nazwy zdarzeń oraz **rdzeń decyzyjny**
//! ([`LockTable`] + sterownik [`Core`]) — SPEC wymaga determinizmu („ta sama sekwencja żądań =
//! ta sama kolejność przyznań”), więc reguły są częścią kontraktu, a `-impl`/`-fake` różnią się
//! tylko zegarem ([`Host`]) i ujściem zdarzeń.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod core;
mod events;
mod graph;
mod lease;
mod multi;
mod ops;
mod query;
mod table;
mod types;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use async_trait::async_trait;

pub use crate::core::{Core, Host};
pub use events::{
    EVENT_CANCELLED, EVENT_DEADLOCK, EVENT_GRANTED, EVENT_HANDOFF, EVENT_PREEMPTED, EVENT_QUEUED,
    EVENT_RELEASED, EVENT_REVOKED, EVENT_TIMEOUT, effect_event, event_kind,
};
pub use graph::WaitEdge;
pub use lease::{Lease, LeaseControl};
pub use table::{Effect, LockTable};
pub use types::{
    Holder, LeaseId, LeaseInfo, LeaseRequest, LeaseSignal, MAX_WAIT_LIMIT, OnTimeout,
    PreemptReason, Priority, QueuedRequest, RequestId, Resource, ResourcePolicy, SchedError,
    millis,
};

/// Scheduler zasobów wyłącznych.
#[async_trait]
pub trait SchedulerLite: Send + Sync {
    /// Żąda zasobu; czeka najwyżej `max_wait` (potem `SchedError::Timeout` z `on_timeout`).
    /// Dzierżawa zwalnia zasób przy `drop`.
    async fn acquire(&self, request: LeaseRequest) -> Result<Lease, SchedError>;

    /// Wywłaszczenie posiadaczki (`UserSpeaks`, `HigherPriority`, `Handoff` → sygnał w punkcie
    /// atomowym; `KillSwitch` → odebranie).
    fn preempt(
        &self,
        resource: &Resource,
        by: Holder,
        reason: PreemptReason,
    ) -> Result<(), SchedError>;

    /// Przekazanie zasobu trzymanego przez `from` bez luki (delegacja v0: „Przekazuję Delcie…”).
    fn handoff(&self, resource: &Resource, from: &Holder, to: Holder) -> Result<(), SchedError>;

    /// Bieżąca posiadaczka zasobu.
    fn holder(&self, resource: &Resource) -> Option<LeaseInfo>;

    /// Kolejka zasobu (kto czeka, w kolejności przyznawania).
    fn queue(&self, resource: &Resource) -> Vec<QueuedRequest>;

    /// Kill-switch: odbiera wszystkie dzierżawy i czyści kolejki. Zwraca liczbę objętych.
    fn kill_all(&self) -> usize;
}
