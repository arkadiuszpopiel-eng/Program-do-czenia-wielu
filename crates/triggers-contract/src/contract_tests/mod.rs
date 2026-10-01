//! Współdzielone testy kontraktowe (feature `contract-tests`) — te same dla `-impl` i `-fake`.
//! Uprząż ustawia początek czasu ([`Harness`] z fabryki `start_ms`), przesuwa go i pokazuje
//! zadania przekazane do schedulera.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::future::Future;

use async_trait::async_trait;
use chrono::NaiveDateTime;
use scheduler_contract::TaskSpec;

use crate::{Actor, TriggerAction, TriggerKind, TriggerSpec, Triggers};

mod events;
mod policy;
mod time;

pub use events::{event_triggers_taint_and_origin, rate_limit_quiet_and_dnd};
pub use policy::{ownership_bridges_and_scope, run_log};
pub use time::{cron_dst_fall_back_once, cron_dst_spring_forward, interval_once_manual};

/// Uprząż testu.
#[async_trait]
pub trait Harness: Send + Sync {
    /// Typ wyzwalaczy.
    type T: Triggers;
    /// Wyzwalacze.
    fn triggers(&self) -> &Self::T;
    /// Przesuwa czas o `ms` (sterownik wyzwala terminy po drodze).
    async fn advance(&self, ms: u64);
    /// Zadania przekazane do schedulera (w kolejności).
    fn submitted(&self) -> Vec<TaskSpec>;
    /// Bieżący czas (ms UTC).
    fn now_ms(&self) -> u64;
}

/// Chwila UTC z tekstu `RRRR-MM-DD GG:MM`.
pub fn utc(s: &str) -> u64 {
    let t = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap();
    u64::try_from(t.and_utc().timestamp_millis()).unwrap()
}

/// Chwile wyzwolenia zgłoszonych zadań (z ładunku).
pub fn fired_at<H: Harness>(h: &H) -> Vec<u64> {
    h.submitted()
        .iter()
        .map(|t| t.payload["trigger"]["fired_at_ms"].as_u64().unwrap())
        .collect()
}

/// Wyzwalacz użytkownika.
pub fn user_trigger(id: &str, kind: TriggerKind) -> TriggerSpec {
    TriggerSpec::new(
        id,
        format!("wyzwalacz {id}"),
        Actor::User,
        kind,
        TriggerAction::new(format!("zadanie {id}"), "uporządkuj pobrane pliki"),
    )
}

/// Cały zestaw; `factory(start_ms)` daje świeżą uprząż z zegarem ustawionym na `start_ms`.
pub async fn run_all<H, F, Fut>(factory: F)
where
    H: Harness,
    F: Fn(u64) -> Fut,
    Fut: Future<Output = H>,
{
    cron_dst_spring_forward(&factory(utc("2026-03-27 00:00")).await).await;
    cron_dst_fall_back_once(&factory(utc("2026-10-24 00:00")).await).await;
    interval_once_manual(&factory(utc("2026-06-01 10:00")).await).await;
    event_triggers_taint_and_origin(&factory(utc("2026-06-01 10:00")).await).await;
    rate_limit_quiet_and_dnd(&factory(utc("2026-06-01 10:00")).await).await;
    ownership_bridges_and_scope(&factory(utc("2026-06-01 10:00")).await).await;
    run_log(&factory(utc("2026-06-01 10:00")).await).await;
}
