//! Zachowania atrapy i rdzenia poza wspólnym zestawem: budżet tła, delegacja (dziedziczenie
//! pochodzenia i taintu), most z wyzwalacza, pętla, steering po ostatnim kroku, restart w trakcie
//! anulowania, stan JSON, obsada, limity, prywatność zdarzeń.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use agent_backends_contract::{BridgeKind, LaunchOrigin};
use cost_meter_contract::{BudgetDecision, BudgetNotice, BudgetScope};
use safety_broker_contract::TaintSource;
use scheduler_contract::contract_tests::Script;
use scheduler_contract::{
    AgentSlot, Assignee, BlockReason, CancelCause, DispatchId, EVENT_BUDGET_WARNING,
    EVENT_DISPATCHED, EVENT_FINISHED, EVENT_LOOP, EVENT_STEER_UNCONSUMED, EVENT_SUBMITTED,
    ExecutorKind, MAX_TASKS_PER_SUBMIT, Roster, Scheduler, Snapshot, Steer, StepDirective,
    StepReport, TaskClass, TaskError, TaskOrigin, TaskSpec, TaskState, Termination,
};
use scheduler_fake::FakeScheduler;

fn spec(id: &str, class: TaskClass) -> TaskSpec {
    TaskSpec::new(id, id, Assignee::AnyAgent, class, TaskOrigin::User)
}

fn notice() -> BudgetNotice {
    BudgetNotice {
        scope: BudgetScope::Background,
        spent_micro_pln: 900,
        estimate_micro_pln: 200,
        limit_micro_pln: 1_000,
        pct_after: 110,
    }
}

#[test]
fn background_budget_from_cost_meter() {
    let f = FakeScheduler::new();
    let mut paid = spec("platne-tlo", TaskClass::Background);
    paid.budget.estimated_cost_micro_pln = 200;
    let mut local = spec("lokalne-tlo", TaskClass::Background);
    local.budget.estimated_cost_micro_pln = 0;
    let mut user = spec("uzytkownik", TaskClass::User);
    user.budget.estimated_cost_micro_pln = 200;
    f.set_background_budget(BudgetDecision::Block { notice: notice() });
    f.submit(vec![paid, local, user]).unwrap();
    f.advance(100);
    assert!(matches!(
        f.task(&"platne-tlo".into()).unwrap().state,
        TaskState::Done {
            termination: Termination::BudgetBlocked { .. }
        }
    ));
    for id in ["lokalne-tlo", "uzytkownik"] {
        assert!(
            f.task(&id.into())
                .unwrap()
                .state
                .termination()
                .unwrap()
                .is_success()
        );
    }
    f.set_background_budget(BudgetDecision::Warn {
        notices: vec![notice()],
    });
    let mut warn = spec("ostrzezenie", TaskClass::Background);
    warn.budget.estimated_cost_micro_pln = 200;
    f.submit(vec![warn]).unwrap();
    f.advance(100);
    assert_eq!(f.events_named(EVENT_BUDGET_WARNING).len(), 1);
    assert!(
        f.task(&"ostrzezenie".into())
            .unwrap()
            .state
            .termination()
            .unwrap()
            .is_success()
    );
}

#[test]
fn delegation_inherits_origin_and_taint_and_bridges_stay_closed() {
    let f = FakeScheduler::new();
    f.script(&"z-wyzwalacza".into(), Script::ok(3, 100));
    let mut parent = spec("z-wyzwalacza", TaskClass::Background);
    parent.origin = TaskOrigin::Trigger {
        trigger_id: "pobrane".into(),
        depth: 1,
    };
    parent.taint = vec![TaintSource::File];
    // Most z wyzwalacza odrzucony już przy zgłoszeniu.
    let mut bridge = parent.clone();
    bridge.id = "most".into();
    bridge.executor = ExecutorKind::Bridge(BridgeKind::ClaudeCode);
    assert!(matches!(
        f.submit(vec![bridge]),
        Err(TaskError::BridgeNotAllowed { .. })
    ));
    f.submit(vec![parent]).unwrap();
    f.advance(50);
    let dispatch: DispatchId = f.core().running()[0].1;
    // Agentka próbuje „wyprać” pochodzenie: podzadanie jako żądanie użytkownika z mostem.
    let mut child = spec("pod-most", TaskClass::User);
    child.executor = ExecutorKind::Bridge(BridgeKind::Codex);
    let err = f.spawn(dispatch, vec![child]).unwrap_err();
    assert!(matches!(err, TaskError::BridgeNotAllowed { .. }), "{err:?}");
    let child = spec("pod", TaskClass::User);
    let ids = f.spawn(dispatch, vec![child]).unwrap();
    let view = f.task(&ids[0]).unwrap();
    assert_eq!(
        view.spec.parent.as_ref().map(|p| p.as_str()),
        Some("z-wyzwalacza")
    );
    assert!(matches!(view.spec.origin, TaskOrigin::Trigger { .. }));
    assert_eq!(
        view.spec.origin.launch_origin(),
        LaunchOrigin::Trigger {
            trigger_id: "pobrane".into()
        }
    );
    assert_eq!(view.spec.taint, vec![TaintSource::File]);
    assert_eq!(
        view.spec.class,
        TaskClass::Background,
        "klasa nie wyżej niż rodzic"
    );
    assert_eq!(
        f.task(&"z-wyzwalacza".into()).unwrap().children,
        vec![ids[0].clone()]
    );
    assert_eq!(
        f.spawn(DispatchId(999), vec![spec("x", TaskClass::User)]),
        Err(TaskError::StaleDispatch(DispatchId(999)))
    );
}

#[test]
fn loop_suspicion_and_unconsumed_steering() {
    let f = FakeScheduler::new();
    f.script(
        &"petla".into(),
        Script {
            fingerprint: Some(42),
            ..Script::ok(6, 10)
        },
    );
    f.script(&"ostatni".into(), Script::ok(2, 100));
    f.submit(vec![
        spec("petla", TaskClass::Agent),
        spec("ostatni", TaskClass::Agent),
    ])
    .unwrap();
    f.advance(150); // „ostatni” w ostatnim kroku
    f.steer(&"ostatni".into(), Steer::text("za późno")).unwrap();
    f.advance(1_000);
    let loops = f.events_named(EVENT_LOOP);
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0].payload["task"], "petla");
    let unconsumed = f.events_named(EVENT_STEER_UNCONSUMED);
    assert_eq!(unconsumed.len(), 1);
    assert_eq!(unconsumed[0].payload["task"], "ostatni");
}

#[test]
fn restart_during_cancellation_and_snapshot_json() {
    let f = FakeScheduler::new();
    f.script(&"wolne".into(), Script::ok(5, 1_000));
    f.submit(vec![spec("wolne", TaskClass::User)]).unwrap();
    f.advance(500);
    f.cancel(&"wolne".into(), "nie trzeba").unwrap();
    // Snapshot przechodzi przez JSON (format trwały).
    let json = serde_json::to_string(&f.stored_snapshot().unwrap()).unwrap();
    let back: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.active_count(), 1);
    f.restart().unwrap();
    assert!(matches!(
        f.task(&"wolne".into()).unwrap().state,
        TaskState::Done {
            termination: Termination::Cancelled {
                cause: CancelCause::User { .. }
            }
        }
    ));
    let mut bad: serde_json::Value = serde_json::from_str(&json).unwrap();
    bad["version"] = serde_json::json!(99);
    let bad: Snapshot = serde_json::from_value(bad).unwrap();
    let host = std::sync::Arc::new(scheduler_fake::FakeHost::default());
    assert!(scheduler_contract::SchedCore::restore(host, bad).is_err());
}

#[test]
fn stale_boundary_stops_worker() {
    let f = FakeScheduler::new();
    let (directive, _) = f.core().boundary(DispatchId(77), &StepReport::default());
    assert!(matches!(directive, StepDirective::Stop { .. }));
}

#[test]
fn roster_change_unblocks_roles() {
    let f = FakeScheduler::new();
    let mut task = spec("tlumaczenie", TaskClass::Agent);
    task.assignee = Assignee::Role("tlumaczka".into());
    f.submit(vec![task]).unwrap();
    f.advance(10);
    assert_eq!(
        f.task(&"tlumaczenie".into()).unwrap().blocked,
        Some(BlockReason::NoAgent)
    );
    let mut roster = Roster::default();
    roster.agents.push(AgentSlot {
        persona: "epsilon".into(),
        roles: vec!["tlumaczka".into()],
        available: true,
        max_parallel: 1,
    });
    f.set_roster(roster);
    f.advance(100);
    let v = f.task(&"tlumaczenie".into()).unwrap();
    assert!(v.state.termination().unwrap().is_success());
    let d = f.dispatches();
    assert_eq!(d[0].agent.as_ref().map(|p| p.as_str()), Some("epsilon"));
}

#[test]
fn limits_and_event_privacy() {
    let f = FakeScheduler::new();
    let many: Vec<TaskSpec> = (0..=MAX_TASKS_PER_SUBMIT)
        .map(|i| spec(&format!("m{i}"), TaskClass::Agent))
        .collect();
    assert!(matches!(f.submit(many), Err(TaskError::Capacity { .. })));
    let mut secret = spec("prywatne", TaskClass::User);
    secret.payload = serde_json::json!({"cel": "hasło do banku 1234"});
    f.submit(vec![secret]).unwrap();
    f.advance(100);
    for name in [EVENT_SUBMITTED, EVENT_DISPATCHED, EVENT_FINISHED] {
        let events = f.events_named(name);
        assert_eq!(events.len(), 1, "{name}");
        assert!(!events[0].payload.to_string().contains("1234"), "{name}");
    }
    assert!(f.steer(&"nieznane".into(), Steer::text("x")).is_err());
    assert!(f.pause(&"prywatne".into()).is_err());
}
