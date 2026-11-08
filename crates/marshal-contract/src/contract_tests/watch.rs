//! Nadzór: eskalacje ze zdarzeń schedulera i wyzwalaczy, deduplikacja, raport dzienny.

use chrono::NaiveDate;
use serde_json::json;

use super::{Harness, ev};
use crate::{EVENT_ESCALATION, Escalation, FindingKind, Marshal};

/// Długa blokada, budżet, pętla, przerwanie, steering po końcu, łańcuch wyzwalaczy — każde
/// eskalowane raz.
pub async fn escalations_from_scheduler_events<H: Harness>(h: &H) {
    let m = h.marshal();
    let t0 = h.now_ms();
    m.observe(&ev(
        "scheduler.task.submitted",
        json!({"task": "a", "title": "Raport kwartalny", "at_ms": t0}),
    ));
    m.observe(&ev(
        "scheduler.task.blocked",
        json!({"task": "a", "at_ms": t0, "reason": {"blocked": "resources", "busy": []}}),
    ));
    let blocked = |h: &H| -> Vec<Escalation> {
        h.events()
            .iter()
            .filter(|e| e.kind.as_str() == EVENT_ESCALATION)
            .filter_map(|e| serde_json::from_value::<Escalation>(e.payload.clone()).ok())
            .filter(|e| matches!(e.kind, FindingKind::Blocked { .. }))
            .collect()
    };
    h.advance(5 * 60_000).await;
    assert!(m.check().is_empty(), "5 min to jeszcze nie zastój");
    assert!(blocked(h).is_empty());
    h.advance(6 * 60_000).await;
    // Przegląd woła sterownik (impl: co minutę) albo test (fake) — eskalacja dokładnie raz.
    let _ = m.check();
    h.advance(1).await;
    let found = blocked(h);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].kind,
        FindingKind::Blocked {
            reason: "resources".into()
        }
    );
    assert!(
        found[0].message.contains("Raport kwartalny"),
        "{}",
        found[0].message
    );
    assert!(m.check().is_empty(), "bez powtórzeń");
    // Blokada „czeka na zależności” to nie zastój.
    m.observe(&ev(
        "scheduler.task.blocked",
        json!({"task": "b", "at_ms": t0, "reason": {"blocked": "dependencies"}}),
    ));
    assert!(m.check().is_empty());
    let now = h.now_ms();
    let finished = m.observe(&ev(
        "scheduler.task.finished",
        json!({"task": "a", "at_ms": now, "result": "budget_exceeded", "budget": "wall"}),
    ));
    assert!(finished[0].message.contains("czasu"));
    assert_eq!(
        m.observe(&ev(
            "scheduler.task.loop_suspected",
            json!({"task": "c", "at_ms": now})
        ))
        .len(),
        1
    );
    assert_eq!(
        m.observe(&ev(
            "scheduler.task.loop_suspected",
            json!({"task": "c", "at_ms": now})
        ))
        .len(),
        0
    );
    assert_eq!(
        m.observe(&ev(
            "scheduler.task.aborted",
            json!({"task": "d", "at_ms": now})
        ))
        .len(),
        1
    );
    assert_eq!(
        m.observe(&ev(
            "scheduler.task.steer_unconsumed",
            json!({"task": "e", "at_ms": now})
        ))
        .len(),
        1
    );
    assert_eq!(
        m.observe(&ev(
            "scheduler.task.retry_scheduled",
            json!({"task": "f", "at_ms": now, "attempt": 2})
        ))
        .len(),
        0
    );
    assert_eq!(
        m.observe(&ev(
            "scheduler.task.retry_scheduled",
            json!({"task": "f", "at_ms": now, "attempt": 3})
        ))
        .len(),
        1
    );
    let chain = m.observe(&ev("triggers.suppressed", json!({"trigger": "po-zadaniu", "at_ms": now, "outcome": {"outcome": "suppressed", "reason": "chain_too_deep"}})));
    assert_eq!(chain.len(), 1);
    let quiet = m.observe(&ev("triggers.suppressed", json!({"trigger": "x", "at_ms": now, "outcome": {"outcome": "suppressed", "reason": "quiet"}})));
    assert!(quiet.is_empty(), "cisza to nie problem");
    h.advance(1).await; // publikacja zdarzeń bywa asynchroniczna
    let published = h
        .events()
        .iter()
        .filter(|e| e.kind.as_str() == EVENT_ESCALATION)
        .count();
    assert_eq!(published, 7);
}

/// Raport dzienny: liczniki wg dnia lokalnego, najczęstsze blokady, tekst po polsku.
pub async fn daily_report<H: Harness>(h: &H) {
    let m = h.marshal();
    let t = h.now_ms();
    for (i, result) in ["succeeded", "succeeded", "failed", "expired", "cancelled"]
        .iter()
        .enumerate()
    {
        let task = format!("t{i}");
        m.observe(&ev(
            "scheduler.task.submitted",
            json!({"task": task, "title": "x", "at_ms": t}),
        ));
        m.observe(&ev(
            "scheduler.task.finished",
            json!({"task": task, "at_ms": t + 1_000, "result": result}),
        ));
    }
    m.observe(&ev(
        "scheduler.task.blocked",
        json!({"task": "t0", "at_ms": t, "reason": {"blocked": "no_agent"}}),
    ));
    let day = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let r = m.daily_report(day);
    assert_eq!(
        (r.submitted, r.succeeded, r.failed, r.expired, r.cancelled),
        (5, 2, 1, 1, 1)
    );
    assert_eq!(r.blockers.get("no_agent"), Some(&1));
    assert!(r.text.contains("ukończone: 2"), "{}", r.text);
    assert!(r.text.contains("brak agentki"), "{}", r.text);
    let empty = m.daily_report(NaiveDate::from_ymd_opt(2026, 9, 30).unwrap());
    assert_eq!(empty.submitted, 0);
}
