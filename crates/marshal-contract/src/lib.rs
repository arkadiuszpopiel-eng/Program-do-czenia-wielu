//! Kontrakt modułu `marshal` — Marszałek (docs/modules/marshal/SPEC.md, PLAN §9.4, F5-09).
//!
//! Marszałek **nie szereguje**: zamienia polecenia w języku naturalnym na deklaratywne reguły
//! ([`Rule`]: `when` → `then`), proponuje je, wykrywa konflikty ([`conflicts`]), a **Ty
//! zatwierdzasz** ([`Approver`] — agentka nigdy). Reguły **tylko zawężają** uprawnienia: język
//! efektów ([`Effect`]) nie ma „pozwól/podnieś/wyłącz”, a parametry są sprawdzane względem
//! bieżącego sufitu ([`Ceiling`], [`check_rule`]); polityka efektywna ([`compose`]) nigdy nie
//! jest szersza niż sufit. Pauza tylko w punktach atomowych i tylko dla zadań GUI/audio.
//!
//! Drugie zadanie (nadzór, [`Watch`]): na zdarzeniach `scheduler.*`/`triggers.*` wykrywa długie
//! blokady, przekroczone budżety, pętle, powtarzane porażki i przerwania, eskaluje do
//! użytkownika (z limitem) i składa raport dzienny — relacjonuje je Dyrygentka.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod book;
mod check;
mod core;
mod events;
mod policy;
mod rule;
mod watch;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use crate::core::{Marshal, MarshalCore, MarshalHost, RuleTranslator};
pub use book::{
    Approver, MAX_DECIDED_PROPOSALS, MAX_PENDING_PROPOSALS, MarshalError, Proposal, ProposalStatus,
    RejectedDraft, RuleBook,
};
pub use check::{
    Ceiling, Conflict, FAMILIES, MAX_EFFECTS, MAX_WAIT_MS, Violation, check_rule, conflicts,
    resource_name,
};
pub use events::{
    EVENT_APPROVED, EVENT_ESCALATION, EVENT_PROPOSED, EVENT_REJECTED, EVENT_REPORT, EVENT_REVOKED,
    escalation_event, event_kind, marshal_event,
};
pub use policy::{EffectivePolicy, compose, within};
pub use rule::{
    Confidence, Effect, EventKind, OriginKind, PauseScope, ResourceKind, ResumeAfter, Rule, RuleId,
    TimeRange, When, parse_rule,
};
pub use watch::{DailyReport, Escalation, FindingKind, Severity, Watch, WatchConfig};
