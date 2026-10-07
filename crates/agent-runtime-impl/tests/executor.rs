//! v1: adapter `TaskExecutor` — oddanie zadania (pauza/wywłaszczenie) i wznowienie z checkpointu,
//! zatrzymanie dyrektywą, ładunek niepoprawny, podmiana agentki z obsady, pochodzenie i taint
//! zadania, odrzucenie przez Krytyczkę = porażka, raporty kroków, sterowanie sprzed startu,
//! porzucenie wykonania anuluje przebieg.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_runtime_contract::{
    AgentRuntime, AgentTaskPayload, BudgetKind, CheckpointStore, MemCheckpointStore, RunEvent,
    RunOptions, RunOutcome, RunStatus,
};
use agent_runtime_impl::{
    Runtime, RuntimeConfig, RuntimeDeps, RuntimeExecutor, adapt_payload, task_run_id,
};
use common::v1::{Routed, Slow, TestGate, dispatch, envelope, spec_for, standard_crew};
use common::{answer, call};
use risk_classifier_contract::CommandOrigin;
use safety_broker_contract::TaintSource;
use scheduler_contract::{
    BudgetKind as TaskBudget, Steer, StepDirective, StopReason, TaskExecutor, TaskId, TaskOrigin,
    WorkerResult, YieldReason,
};
use serde_json::json;
use tools_common_contract::Tool;
use tools_fs_contract::FsToolKind;

struct W {
    p: Routed,
    rt: Arc<Runtime>,
    store: MemCheckpointStore,
}

fn world() -> W {
    let p = Routed::new(&["m-delta", "m-gama"]);
    let spans = Arc::new(Mutex::new(Vec::new()));
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(Slow::new(FsToolKind::Write, 50, spans.clone())),
        Arc::new(Slow::new(FsToolKind::Read, 50, spans)),
    ];
    let store = MemCheckpointStore::default();
    let rt = Runtime::new(RuntimeDeps {
        provider: Arc::new(p.clone()),
        tools,
        checkpoints: Arc::new(store.clone()),
        bus: None,
        config: RuntimeConfig::default(),
    });
    W {
        p,
        rt: Arc::new(rt),
        store,
    }
}

fn writes(w: &W, n: usize) {
    for i in 0..n {
        w.p.model("m-delta").push_script(call(
            &format!("w{i}"),
            "fs_write",
            json!({"path": format!("/u/{i}.md"), "content": "x"}),
        ));
    }
    w.p.model("m-delta").push_script(answer("Gotowe."));
}

fn delta() -> agent_runtime_contract::RunSpec {
    spec_for("delta", "operator", &["fs_write", "fs_read"])
}

#[tokio::test(start_paused = true)]
async fn yield_then_resume_from_checkpoint() {
    let w = world();
    writes(&w, 3);
    let exec = RuntimeExecutor::new(&w.rt, 40_000);
    let gate = TestGate::new();
    gate.at(
        1,
        StepDirective::Yield {
            reason: YieldReason::Preempted { resource: None },
        },
    );
    let d = dispatch("zad-1", 1, delta(), RunOptions::default(), TaskOrigin::User);
    assert_eq!(
        exec.execute(d.clone(), gate.clone()).await,
        WorkerResult::Yielded
    );
    let run = task_run_id(&TaskId::new("zad-1"), 1);
    assert!(matches!(
        w.rt.status(&run).unwrap(),
        RunStatus::Paused { .. }
    ));
    let saved = w.store.latest(&run).unwrap().unwrap();
    assert!(saved.finished.is_none() && saved.pending.is_empty());
    let mut again = d;
    again.resume_from_step = saved.usage.steps;
    let gate2 = TestGate::new();
    let result = exec.execute(again, gate2.clone()).await;
    let WorkerResult::Succeeded { output } = result else {
        panic!("{result:?}")
    };
    assert_eq!(output.summary, "Gotowe.");
    assert_eq!(output.values["run"], json!(run));
    let names: Vec<&str> =
        w.rt.events(&run)
            .unwrap()
            .iter()
            .map(|e| e.event.name())
            .collect();
    assert!(names.contains(&"agent.run.paused") && names.contains(&"agent.run.resumed"));
    let reqs = w.p.model("m-delta").requests();
    reqs.iter().for_each(|r| r.validate().unwrap());
    assert!(reqs.iter().any(|r| {
        serde_json::to_string(&r.messages)
            .unwrap()
            .contains("oddane schedulerowi")
    }));
    assert_eq!(gate.calls(), 1);
    assert!(
        gate2
            .reports
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.fingerprint.is_some()),
        "odcisk kroku narzędzia"
    );
}

#[tokio::test(start_paused = true)]
async fn stop_directive_stops_with_budget() {
    let w = world();
    writes(&w, 3);
    let gate = TestGate::new();
    gate.at(
        1,
        StepDirective::Stop {
            reason: StopReason::Budget {
                budget: TaskBudget::Cost,
            },
        },
    );
    let exec = RuntimeExecutor::new(&w.rt, 40_000);
    let d = dispatch("zad-2", 1, delta(), RunOptions::default(), TaskOrigin::User);
    assert_eq!(exec.execute(d, gate).await, WorkerResult::Stopped);
    let run = task_run_id(&TaskId::new("zad-2"), 1);
    assert_eq!(
        w.rt.wait(&run).await.unwrap(),
        RunOutcome::BudgetExceeded {
            budget: BudgetKind::Cost
        }
    );
}

#[tokio::test(start_paused = true)]
async fn bad_payload_and_missing_crew_fail_without_retry() {
    let w = world();
    let exec = RuntimeExecutor::new(&w.rt, 40_000);
    let mut d = dispatch("zad-3", 1, delta(), RunOptions::default(), TaskOrigin::User);
    d.spec.payload = json!({"x": 1});
    assert!(matches!(
        exec.execute(d, TestGate::new()).await,
        WorkerResult::Failed {
            retryable: false,
            ..
        }
    ));
    let mut d = dispatch("zad-4", 1, delta(), RunOptions::default(), TaskOrigin::User);
    d.agent = Some(personas_contract::PersonaId::beta());
    assert!(matches!(
        exec.execute(d, TestGate::new()).await,
        WorkerResult::Failed {
            retryable: false,
            ..
        }
    ));
}

#[tokio::test(start_paused = true)]
async fn assigned_persona_replaces_payload_persona() {
    let w = world();
    w.p.model("m-delta").push_script(answer("Beta: gotowe."));
    let exec = RuntimeExecutor::new(&w.rt, 40_000);
    let opts = RunOptions {
        crew: Some(standard_crew(&[])),
        ..RunOptions::default()
    };
    let mut d = dispatch("zad-5", 1, delta(), opts, TaskOrigin::User);
    d.agent = Some(personas_contract::PersonaId::beta());
    assert!(matches!(
        exec.execute(d, TestGate::new()).await,
        WorkerResult::Succeeded { .. }
    ));
    let sys = w.p.model("m-delta").requests()[0].system.clone().unwrap();
    assert!(sys.contains("Beta") && sys.contains("Strażniczka"), "{sys}");
}

#[test]
fn adapt_payload_inherits_origin_taint_and_budget() {
    let mut d = dispatch(
        "zad-6",
        1,
        delta(),
        RunOptions::default(),
        TaskOrigin::Trigger {
            trigger_id: "t".into(),
            depth: 1,
        },
    );
    d.spec.taint = vec![TaintSource::Email];
    d.spec.budget.max_steps = 5;
    let payload: AgentTaskPayload = serde_json::from_value(d.spec.payload.clone()).unwrap();
    let out = adapt_payload(payload, &d).unwrap();
    assert_eq!(
        out.spec.origin,
        CommandOrigin::Agent,
        "wyzwalacz nie udaje użytkownika"
    );
    assert_eq!(out.options.inherited_taint, Some(TaintSource::Email));
    assert_eq!(out.spec.budget.max_steps, 5);
}

#[tokio::test(start_paused = true)]
async fn critic_rejection_is_a_non_retryable_failure() {
    let w = world();
    for _ in 0..2 {
        w.p.model("m-delta").push_script(answer("Gotowe."));
        w.p.model("m-gama")
            .push_script(answer("WERYFIKACJA: BŁĄD — brak pliku"));
    }
    let mut spec = delta();
    spec.verify = true;
    let opts = RunOptions {
        crew: Some(standard_crew(&[("critic", "m-gama")])),
        ..RunOptions::default()
    };
    let d = dispatch("zad-7", 1, spec, opts, TaskOrigin::User);
    let r = RuntimeExecutor::new(&w.rt, 40_000)
        .execute(d, TestGate::new())
        .await;
    assert!(
        matches!(r, WorkerResult::Failed { retryable: false, ref error } if error.contains("Krytyczka")),
        "{r:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn steering_sent_before_start_reaches_first_turn() {
    let w = world();
    w.p.model("m-delta").push_script(answer("Gotowe."));
    let mut d = dispatch("zad-8", 1, delta(), RunOptions::default(), TaskOrigin::User);
    d.steering = vec![envelope(1, Steer::text("Najpierw PDF-y"))];
    RuntimeExecutor::new(&w.rt, 40_000)
        .execute(d, TestGate::new())
        .await;
    let first = &w.p.model("m-delta").requests()[0];
    assert!(
        first
            .messages
            .iter()
            .any(|m| m.visible_text().contains("Najpierw PDF-y"))
    );
}

#[tokio::test(start_paused = true)]
async fn dropped_execution_cancels_the_run() {
    let w = world();
    w.p.model("m-delta")
        .push_script(providers_fake::Script::stall());
    let exec = RuntimeExecutor::new(&w.rt, 40_000);
    let d = dispatch("zad-9", 1, delta(), RunOptions::default(), TaskOrigin::User);
    let r =
        tokio::time::timeout(Duration::from_millis(100), exec.execute(d, TestGate::new())).await;
    assert!(r.is_err(), "wykonanie porzucone");
    let run = task_run_id(&TaskId::new("zad-9"), 1);
    assert_eq!(w.rt.wait(&run).await.unwrap(), RunOutcome::Cancelled);
    assert!(
        w.rt.events(&run)
            .unwrap()
            .iter()
            .any(|e| matches!(e.event, RunEvent::Finished { .. }))
    );
}
