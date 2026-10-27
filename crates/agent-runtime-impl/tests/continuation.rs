//! Wznowienie po limicie zachowuje skutki i historię, anulowania nie reaktywuje.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use agent_runtime_contract::{AgentRuntime, RunBudget, RunOutcome};
use common::{answer, call, spec, world};
use serde_json::json;
use tools_fs_contract::FsToolKind;
#[tokio::test(start_paused = true)]
async fn extend_saved_run_without_repeating_completed_write() {
    let w = world();
    let mut s = spec();
    s.verify = false;
    s.budget.max_steps = 2;
    w.provider.push_script(call(
        "w1",
        "fs_write",
        json!({"path":"/Users/ala/Documents/a","content":"done","mode":"create"}),
    ));
    let run = w.runtime.start(s).await.unwrap();
    assert!(matches!(
        w.runtime.wait(&run).await.unwrap(),
        RunOutcome::BudgetExceeded { .. }
    ));
    w.provider.push_script(answer("Dokończone."));
    w.runtime
        .extend_and_resume(
            &run,
            RunBudget {
                max_steps: 10,
                ..RunBudget::default()
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        w.runtime.wait(&run).await.unwrap(),
        RunOutcome::Completed { .. }
    ));
    assert_eq!(w.fs.calls(FsToolKind::Write).len(), 1);
    assert!(
        w.provider
            .requests()
            .last()
            .unwrap()
            .messages
            .iter()
            .any(|m| m
                .content
                .iter()
                .any(|b| matches!(b, providers_contract::ContentBlock::ToolResult(_))))
    );
}

fn slow_answer() -> providers_fake::Script {
    use providers_contract::{ProviderEvent, StopReason};
    use providers_fake::Step;
    providers_fake::Script::new(vec![
        Step::Emit(ProviderEvent::TextDelta {
            index: 0,
            text: "Saved partial response".into(),
        }),
        Step::Delay(std::time::Duration::from_secs(2)),
        Step::Emit(ProviderEvent::stop(StopReason::EndTurn)),
    ])
}
#[tokio::test(start_paused = true)]
async fn time_limit_during_model_stream_preserves_partial_progress() {
    use agent_runtime_contract::CheckpointStore;
    let w = world();
    let mut s = spec();
    s.verify = false;
    s.budget.max_wall_ms = 1000;
    w.provider.push_script(slow_answer());
    let id = w.runtime.start(s).await.unwrap();
    assert!(matches!(
        w.runtime.wait(&id).await.unwrap(),
        RunOutcome::BudgetExceeded {
            budget: agent_runtime_contract::BudgetKind::Wall
        }
    ));
    assert!(
        w.store
            .latest(&id)
            .unwrap()
            .unwrap()
            .messages
            .iter()
            .any(|m| m.visible_text().contains("Saved partial response"))
    );
}
#[tokio::test(start_paused = true)]
async fn automatic_time_extension_allows_long_model_stream() {
    use agent_runtime_contract::{CheckpointStore, ContinuationPolicy, RunOptions};
    let w = world();
    let mut s = spec();
    s.verify = false;
    s.budget.max_wall_ms = 1000;
    w.provider.push_script(slow_answer());
    let id = w
        .runtime
        .start_with(
            s,
            RunOptions {
                continuation: ContinuationPolicy {
                    auto_time: true,
                    time_chunk_ms: 1000,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        w.runtime.wait(&id).await.unwrap(),
        RunOutcome::Completed { .. }
    ));
    assert!(
        w.store
            .latest(&id)
            .unwrap()
            .unwrap()
            .spec
            .budget
            .max_wall_ms
            > 1000
    );
}

#[tokio::test(start_paused = true)]
async fn failed_local_checkpoint_prevents_mutation() {
    use agent_runtime_contract::{
        Checkpoint, CheckpointError, CheckpointStore, MemCheckpointStore, RunId,
    };
    use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps};
    use std::sync::Arc;
    use tools_common_contract::Toolset;
    struct FullDisk(MemCheckpointStore);
    impl CheckpointStore for FullDisk {
        fn save(&self, c: &Checkpoint) -> Result<(), CheckpointError> {
            if !c.started_pending.is_empty() {
                Err(CheckpointError("disk full".into()))
            } else {
                self.0.save(c)
            }
        }
        fn latest(&self, r: &RunId) -> Result<Option<Checkpoint>, CheckpointError> {
            self.0.latest(r)
        }
        fn runs(&self) -> Result<Vec<RunId>, CheckpointError> {
            self.0.runs()
        }
    }
    let w = world();
    let rt = Runtime::new(RuntimeDeps {
        provider: Arc::new(w.provider.clone()),
        tools: w.fs.tools(),
        checkpoints: Arc::new(FullDisk(MemCheckpointStore::default())),
        bus: None,
        config: RuntimeConfig::default(),
    });
    let mut s = spec();
    s.verify = false;
    w.provider.push_script(call(
        "w",
        "fs_write",
        json!({"path":"/Users/ala/Documents/a","content":"x","mode":"create"}),
    ));
    let run = rt.start(s).await.unwrap();
    assert!(matches!(
        rt.wait(&run).await.unwrap(),
        RunOutcome::Failed { .. }
    ));
    assert!(w.fs.calls(FsToolKind::Write).is_empty());
}

#[tokio::test(start_paused = true)]
async fn known_snapshot_failure_does_not_spin_through_model_turns() {
    use agent_runtime_contract::{Checkpoint, CheckpointStore, RunId};
    let w = world();
    let mut s = spec();
    s.verify = false;
    let run = RunId::new("saved-snapshot-failure");
    let args = json!({"path":"/Users/ala/Documents/a","content":"x","mode":"create"});
    let mut cp = Checkpoint::initial(run.clone(), s);
    cp.failed_preflight.insert(format!(
        "fs_write:{}",
        serde_json::to_string(&args).unwrap()
    ));
    w.store.save(&cp).unwrap();
    w.provider.push_script(call("same", "fs_write", args));
    w.provider.push_script(answer("Kolejna kosztowna próba"));
    w.runtime.resume(&run).await.unwrap();
    let result = w.runtime.wait(&run).await.unwrap();
    assert!(
        matches!(result, RunOutcome::LoopDetected { .. }),
        "{result:?}"
    );
    assert!(w.fs.calls(FsToolKind::Write).is_empty());
    assert_eq!(w.provider.requests().len(), 1);
}
