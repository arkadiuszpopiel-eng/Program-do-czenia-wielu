//! F5-02 / ACC-F5-agent-runtime-04: steering uwzględniony w ≤ 1 kroku atomowym — 20/20
//! (tekst i głos; zestaw: `evals/F5/steering-cases.json`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use scheduler_contract::contract_tests::Script;
use scheduler_contract::{
    Assignee, EVENT_STEERED, Holder, LeaseRequest, Priority, Resource, Scheduler, Steer, SteerVia,
    TaskClass, TaskOrigin, TaskSpec,
};
use scheduler_fake::FakeScheduler;

#[derive(serde::Deserialize)]
struct Case {
    id: String,
    via: String,
    kind: String,
    start: String,
    steps: u32,
    step_ms: u64,
    send_at_ms: u64,
    text: String,
}

#[derive(serde::Deserialize)]
struct Set {
    threshold: Threshold,
    cases: Vec<Case>,
}

#[derive(serde::Deserialize)]
struct Threshold {
    passed: usize,
    of: usize,
}

fn poll<F: Future + Unpin>(fut: &mut F) -> Option<F::Output> {
    let mut cx = Context::from_waker(Waker::noop());
    match Pin::new(fut).poll(&mut cx) {
        Poll::Ready(v) => Some(v),
        Poll::Pending => None,
    }
}

/// Uruchamia przypadek; `Ok(())` = steering dostarczony przed oczekiwanym krokiem, z opóźnieniem ≤ 1.
fn run(case: &Case) -> Result<(), String> {
    let f = FakeScheduler::new();
    let id = case.id.as_str().into();
    f.script(&id, Script::ok(case.steps, case.step_ms));
    let mut spec = TaskSpec::new(
        case.id.as_str(),
        "zadanie sterowane",
        Assignee::AnyAgent,
        TaskClass::Agent,
        TaskOrigin::User,
    );
    if case.start == "queued" {
        spec.window.not_before_ms = Some(f.now_ms() + 1_000);
    }
    if case.start == "preempted" {
        spec.resources.push(Resource::Speaker);
    }
    f.submit(vec![spec]).map_err(|e| e.to_string())?;
    f.advance(case.send_at_ms);
    let via = if case.via == "voice" {
        SteerVia::Voice
    } else {
        SteerVia::Text
    };
    let steer = match case.kind.as_str() {
        "change_goal" => Steer::ChangeGoal {
            goal: case.text.clone(),
            via,
        },
        _ => Steer::Message {
            text: case.text.clone(),
            via,
        },
    };
    let mut voice = None;
    match case.start.as_str() {
        "paused" => drop(
            f.steer(&id, Steer::PauseAfterCurrent)
                .map_err(|e| e.to_string())?,
        ),
        "preempted" => {
            let req = LeaseRequest::new(
                Resource::Speaker,
                Holder::User,
                Priority::UserSpeech,
                Duration::from_secs(5),
            );
            let locks = std::sync::Arc::clone(f.core().locks());
            let mut fut = Box::pin(async move { locks.acquire(req).await });
            // Pierwsze odpytanie rejestruje żądanie (głośnik trzyma zadanie → czeka).
            if poll(&mut fut).is_some() {
                return Err("głośnik powinien być zajęty przez zadanie".into());
            }
            voice = Some(fut);
        }
        _ => {}
    }
    let seq = f.steer(&id, steer).map_err(|e| e.to_string())?;
    if case.start == "paused" {
        f.advance(case.step_ms + 500);
        f.resume(&id).map_err(|e| e.to_string())?;
    }
    if let Some(fut) = voice.as_mut() {
        f.advance(case.step_ms);
        let lease = poll(fut)
            .ok_or("mowa nie dostała głośnika")?
            .map_err(|e| e.to_string())?;
        f.advance(300);
        drop(lease);
    }
    f.advance(u64::from(case.steps) * case.step_ms * 4 + 1_000);
    let expected = if case.start == "queued" {
        1
    } else {
        u32::try_from(case.send_at_ms / case.step_ms).unwrap_or(u32::MAX) + 2
    };
    let seen = f.seen_steering(&id);
    let hit = seen
        .iter()
        .find(|(_, e)| e.seq == seq)
        .ok_or("steering niedostarczony")?;
    if hit.0 != expected {
        return Err(format!(
            "dostarczony przed krokiem {}, oczekiwano {expected}",
            hit.0
        ));
    }
    for e in f.events_named(EVENT_STEERED) {
        let latency = e.payload["latency_steps"].as_u64().unwrap_or(u64::MAX);
        if latency > 1 {
            return Err(format!("opóźnienie {latency} kroków"));
        }
    }
    let view = f.task(&id).ok_or("brak zadania")?;
    if !view.state.termination().is_some_and(|t| t.is_success()) {
        return Err(format!(
            "zadanie nie zakończyło się sukcesem: {:?}",
            view.state
        ));
    }
    Ok(())
}

#[test]
fn steering_within_one_atomic_step_20_of_20() {
    let set: Set =
        serde_json::from_str(include_str!("../../../evals/F5/steering-cases.json")).unwrap();
    assert_eq!(set.cases.len(), set.threshold.of);
    let mut passed = 0;
    let mut failures = Vec::new();
    for case in &set.cases {
        match run(case) {
            Ok(()) => passed += 1,
            Err(e) => failures.push(format!("{}: {e}", case.id)),
        }
    }
    eprintln!("F5-02: steering ≤ 1 krok: {passed}/{}", set.cases.len());
    assert!(failures.is_empty(), "{failures:?}");
    assert!(passed >= set.threshold.passed);
}
