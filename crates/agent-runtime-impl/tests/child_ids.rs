//! Podprzebieg po wznowieniu dostaje nowy numer — nie nadpisuje checkpointu ukończonego dziecka.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};

use agent_runtime_contract::{
    AgentRuntime, CheckpointStore, MemCheckpointStore, RunBudget, RunId, RunOptions, RunOutcome,
};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps};
use common::v1::{Routed, Slow, spec_for, standard_crew};
use common::{answer, call};
use serde_json::json;
use tools_common_contract::Tool;
use tools_fs_contract::FsToolKind;

#[tokio::test(start_paused = true)]
async fn child_started_after_extension_keeps_earlier_child_checkpoint() {
    let p = Routed::new(&["m-alfa", "m-delta"]);
    let spans = Arc::new(Mutex::new(Vec::new()));
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(Slow::new(FsToolKind::Write, 10, spans.clone())),
        Arc::new(Slow::new(FsToolKind::Read, 10, spans)),
    ];
    let store = Arc::new(MemCheckpointStore::default());
    let rt = Runtime::new(RuntimeDeps {
        provider: Arc::new(p.clone()),
        tools,
        checkpoints: store.clone(),
        bus: None,
        config: RuntimeConfig::default(),
    });
    let mut s = spec_for("alfa", "conductor", &["fs_read", "fs_write"]);
    s.verify = false;
    s.budget.max_wall_ms = 10_000;
    let options = RunOptions {
        crew: Some(standard_crew(&[("operator", "m-delta")])),
        ..RunOptions::default()
    };
    let delegate = |id: &str, goal: &str| {
        call(
            id,
            "delegate_task",
            json!({"role": "operator", "goal": goal}),
        )
    };
    p.model("m-alfa").push_script(delegate("d1", "Pierwsze"));
    p.model("m-delta").push_script(answer("Pierwsze gotowe."));
    // Druga tura Dyrygentki przekracza limit czasu — przebieg czeka na kolejną pulę.
    p.model("m-alfa")
        .push_script(providers_fake::Script::new(vec![
            providers_fake::Step::Delay(std::time::Duration::from_secs(20)),
            providers_fake::Step::Emit(providers_contract::ProviderEvent::stop(
                providers_contract::StopReason::EndTurn,
            )),
        ]));
    let run = rt.start_with(s, options).await.unwrap();
    assert!(matches!(
        rt.wait(&run).await.unwrap(),
        RunOutcome::BudgetExceeded { .. }
    ));
    p.model("m-alfa").push_script(delegate("d2", "Drugie"));
    p.model("m-alfa").push_script(answer("Oba gotowe."));
    p.model("m-delta").push_script(answer("Drugie gotowe."));
    rt.extend_and_resume(
        &run,
        RunBudget {
            max_steps: 10,
            max_wall_ms: 600_000,
            ..RunBudget::default()
        },
    )
    .await
    .unwrap();
    rt.wait(&run).await.unwrap();
    let first = RunId::new(format!("{}-c1", run.as_str()));
    let second = RunId::new(format!("{}-c2", run.as_str()));
    let goal_of = |id: &RunId| store.latest(id).unwrap().map(|cp| cp.spec.goal);
    assert!(goal_of(&first).is_some_and(|g| g.contains("Pierwsze")));
    assert!(goal_of(&second).is_some_and(|g| g.contains("Drugie")));
}
