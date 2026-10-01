//! Kontrakt modułu `triggers` (docs/modules/triggers/SPEC.md, PLAN §9.5, §1.3 pkt 4, §16.2 F5).
//!
//! Wyzwalacze tworzą zadania dla schedulera:
//! - **czasowe** — cron w strefie wyzwalacza z obsługą DST ([`CronExpr`], [`Tz`];
//!   domyślnie `Europe/Warsaw`), jednorazowe, interwały; zaległe wg [`MisfirePolicy`];
//! - **zdarzeniowe** — nowy plik w katalogu (przez port obserwacji plików), nowa wiadomość,
//!   koniec zadania (z ochroną przed pętlą łańcucha); **ręczne**.
//!
//! Każdy wyzwalacz ma właściciela, sufit uprawnień (`scope` — tokeny wydaje Broker przy
//! wykonaniu, nie przy tworzeniu), limit częstości, okno ciszy i DND oraz dziennik uruchomień.
//! **Twarda reguła zgodności:** wyzwalacz nigdy nie uruchamia mostu CLI — zadania mają
//! pochodzenie `Trigger` (most odmawia, `LaunchOrigin::Trigger`); jedyny wyjątek to harmonogram
//! czasowy utworzony przez użytkownika z jawnym `allow_bridges` i limitem dziennym (pochodzenie
//! `Schedule`; zgodę per trasa sprawdza `agent-backends`). Treść, która wyzwoliła (plik,
//! wiadomość), jest **niezaufana** — zadanie dostaje taint i treść osobno od celu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod core;
mod cron;
mod engine;
mod events;
mod record;
mod spec;
mod tz;
mod validate;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use crate::core::{TriggerHost, Triggers, TriggersCore};
pub use cron::{CronError, CronExpr};
pub use engine::{TRIGGERS_SNAPSHOT_VERSION, TaskSink, TriggerEngine, TriggerSnapshot, task_for};
pub use events::{
    EVENT_CREATED, EVENT_DEFERRED, EVENT_FAILED, EVENT_FIRED, EVENT_REMOVED, EVENT_SUPPRESSED,
    EVENT_TOGGLED, EVENT_UPDATED, event_kind, run_event,
};
pub use record::{
    FireCause, RunOutcome, RunRecord, SuppressReason, TriggerInput, TriggerView, file_matches,
    glob_match, norm_path,
};
pub use spec::{
    Actor, FinishFilter, MisfirePolicy, QuietHours, QuietMode, RateLimit, TriggerAction, TriggerId,
    TriggerKind, TriggerSpec,
};
pub use tz::{LocalTime, Tz, eu_dst_bounds};
pub use validate::{
    DAY_MS, DEFAULT_TASK_DEADLINE_MS, GLOBAL_MAX_FIRES_PER_HOUR, MAX_BRIDGE_FIRES_PER_DAY,
    MAX_CHAIN_DEPTH, MAX_INTERVAL_MS, MAX_TRIGGERS, MIN_INTERVAL_MS, TriggerError, may_manage,
    validate,
};
