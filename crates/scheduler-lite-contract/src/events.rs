//! Zdarzenia magistrali `scheduler.lease.*` (wspólne dla `-impl` i `-fake`).

use core_bus_contract::{AgentId, Event, EventKind, Level};
use serde_json::json;

use crate::table::Effect;
use crate::types::{Holder, LeaseInfo};

/// Przyznano dzierżawę.
pub const EVENT_GRANTED: &str = "scheduler.lease.granted";
/// Zwolniono dzierżawę.
pub const EVENT_RELEASED: &str = "scheduler.lease.released";
/// Poproszono posiadaczkę o zwolnienie (wywłaszczenie w punkcie atomowym).
pub const EVENT_PREEMPTED: &str = "scheduler.lease.preempted";
/// Żądanie przekroczyło `max_wait` (`on_timeout`: `ask_user` → UI pyta użytkownika).
pub const EVENT_TIMEOUT: &str = "scheduler.lease.timeout";
/// Żądanie w kolejce (panel Agentki: kto czeka na głos).
pub const EVENT_QUEUED: &str = "scheduler.lease.queued";
/// Przekazanie bez luki.
pub const EVENT_HANDOFF: &str = "scheduler.lease.handoff";
/// Odrzucenie najmłodszego żądania w cyklu oczekiwania.
pub const EVENT_DEADLOCK: &str = "scheduler.lease.deadlock";
/// Dzierżawa odebrana (kill-switch).
pub const EVENT_REVOKED: &str = "scheduler.lease.revoked";
/// Żądanie anulowane.
pub const EVENT_CANCELLED: &str = "scheduler.lease.cancelled";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

fn with_agent(event: Event, holder: &Holder) -> Event {
    match holder {
        Holder::Persona(p) => event.with_agent(AgentId::new(p.as_str())),
        Holder::User | Holder::System(_) => event,
    }
}

fn lease_json(lease: &LeaseInfo, now_ms: u64) -> serde_json::Value {
    json!({
        "lease": lease.id.0,
        "request": lease.request.0,
        "resource": lease.resource,
        "holder": lease.holder,
        "priority": lease.priority,
        "held_ms": now_ms.saturating_sub(lease.granted_at_ms),
    })
}

/// Zdarzenie dla efektu decyzji.
pub fn effect_event(effect: &Effect, now_ms: u64) -> Event {
    let (name, level, holder, payload) = match effect {
        Effect::Granted(l) => (
            EVENT_GRANTED,
            Level::Debug,
            &l.holder,
            lease_json(l, now_ms),
        ),
        Effect::Released(l) => (
            EVENT_RELEASED,
            Level::Debug,
            &l.holder,
            lease_json(l, now_ms),
        ),
        Effect::PreemptRequested { lease, by, reason } => {
            let mut p = lease_json(lease, now_ms);
            p["by"] = json!(by);
            p["reason"] = json!(reason);
            (EVENT_PREEMPTED, Level::Info, &lease.holder, p)
        }
        Effect::HandedOff { lease, to } => {
            let mut p = lease_json(lease, now_ms);
            p["to"] = json!(to);
            (EVENT_HANDOFF, Level::Info, &lease.holder, p)
        }
        Effect::Revoked(lease, reason) => {
            let mut p = lease_json(lease, now_ms);
            p["reason"] = json!(reason);
            (EVENT_REVOKED, Level::Warn, &lease.holder, p)
        }
        Effect::Queued {
            request,
            resource,
            position,
        } => (
            EVENT_QUEUED,
            Level::Debug,
            &request.holder,
            json!({ "request": request.id.0, "resource": resource, "holder": request.holder,
                    "priority": request.priority, "position": position, "deadline_ms": request.deadline_ms }),
        ),
        Effect::TimedOut {
            request,
            resource,
            on_timeout,
        } => (
            EVENT_TIMEOUT,
            Level::Warn,
            &request.holder,
            json!({ "request": request.id.0, "resource": resource, "holder": request.holder,
                    "on_timeout": on_timeout, "waited_ms": now_ms.saturating_sub(request.enqueued_at_ms) }),
        ),
        Effect::Deadlock {
            request,
            resource,
            cycle,
        } => (
            EVENT_DEADLOCK,
            Level::Warn,
            &request.holder,
            json!({ "request": request.id.0, "resource": resource, "holder": request.holder, "cycle": cycle }),
        ),
        Effect::Cancelled { request, resource } => (
            EVENT_CANCELLED,
            Level::Debug,
            &request.holder,
            json!({ "request": request.id.0, "resource": resource, "holder": request.holder }),
        ),
    };
    with_agent(Event::new(event_kind(name), level, payload), holder)
}
