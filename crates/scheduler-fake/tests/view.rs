//! Widok zadania po restarcie: agentka i pierwszy start zakończonego zadania przeżywają restart;
//! stan zapisany przed dodaniem tych pól (bez kluczy w JSON) wczytuje się bez błędu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use scheduler_contract::contract_tests::Script;
use scheduler_contract::{Assignee, Roster, Scheduler, Snapshot, TaskClass, TaskOrigin, TaskSpec};
use scheduler_fake::FakeScheduler;

/// Gama z domyślnej obsady (bez zależności od `personas-contract`).
fn gama() -> Assignee {
    Assignee::Persona(Roster::default().agents[2].persona.clone())
}

fn spec(id: &str) -> TaskSpec {
    TaskSpec::new(id, id, gama(), TaskClass::User, TaskOrigin::User)
}

#[test]
fn finished_view_survives_restart() {
    let s = FakeScheduler::new();
    s.script(&"raport".into(), Script::ok(2, 100));
    s.submit(vec![spec("raport")]).unwrap();
    s.advance(1_000);
    let before = s.task(&"raport".into()).unwrap();
    assert!(before.state.is_terminal());
    assert_eq!(before.agent.clone().map(Assignee::Persona), Some(gama()));
    assert!(before.started_at_ms.is_some());
    s.restart().unwrap();
    let after = s.task(&"raport".into()).unwrap();
    assert_eq!(after.agent, before.agent);
    assert_eq!(after.started_at_ms, before.started_at_ms);
}

#[test]
fn snapshot_without_new_fields_still_loads() {
    let s = FakeScheduler::new();
    s.script(&"stare".into(), Script::ok(1, 10));
    s.submit(vec![spec("stare")]).unwrap();
    s.advance(100);
    let mut json = serde_json::to_value(s.core().snapshot()).unwrap();
    let tasks = json["state"]["tasks"].as_object_mut().unwrap();
    for rec in tasks.values_mut() {
        let rec = rec.as_object_mut().unwrap();
        assert!(rec.remove("first_started_at_ms").is_some());
        assert!(rec.remove("last_agent").is_some());
    }
    let old: Snapshot = serde_json::from_value(json).unwrap();
    assert_eq!(old.task_count(), 1);
    let host = std::sync::Arc::new(scheduler_fake::FakeHost::default());
    let core = scheduler_contract::SchedCore::restore(host, old).unwrap();
    let view = core.task(&"stare".into()).unwrap();
    assert!(view.state.is_terminal());
    assert_eq!((view.agent, view.started_at_ms), (None, None));
}
