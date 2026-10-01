//! Budżety (kroki, tokeny, czas), detektor pętli, anulowanie w trakcie modelu i narzędzia,
//! steering (wiadomość, pauza/wznowienie, zmiana celu), checkpoint i wznowienie po restarcie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use agent_runtime_contract::{
    AgentRuntime, BudgetKind, CheckpointStore, MemCheckpointStore, RunEvent, RunOutcome, RunStatus,
    Steer,
};
use agent_runtime_impl::{DirCheckpointStore, Runtime, RuntimeConfig, RuntimeDeps};
use async_trait::async_trait;
use common::{answer, call, spec, world, world_with};
use providers_contract::ContentBlock;
use providers_fake::Script;
use serde_json::json;
use tools_common_contract::{Tool, ToolCtx, ToolManifest, ToolOutcome, Toolset};
use tools_fs_contract::{FsToolKind, manifest};

fn lists(w: &common::World, n: usize) {
    for i in 0..n {
        w.provider.push_script(call(
            &format!("t{i}"),
            "fs_list",
            json!({"path": format!("/Users/ala/d{i}")}),
        ));
    }
}

fn history_is_valid(w: &common::World) {
    for r in w.provider.requests() {
        r.validate().unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn step_budget_stops_cleanly() {
    let w = world();
    lists(&w, 20);
    let mut s = spec();
    s.budget.max_steps = 5;
    let run = w.runtime.start(s).await.unwrap();
    assert_eq!(
        w.runtime.wait(&run).await.unwrap(),
        RunOutcome::BudgetExceeded {
            budget: BudgetKind::Steps
        }
    );
    let events = w.runtime.events(&run).unwrap();
    assert!(events.iter().any(|e| matches!(
        e.event,
        RunEvent::BudgetExceeded {
            budget: BudgetKind::Steps
        }
    )));
    let cp = w.store.latest(&run).unwrap().unwrap();
    assert!(cp.finished.is_some() && cp.pending.is_empty() && cp.usage.steps <= 5);
    history_is_valid(&w);
}

#[tokio::test(start_paused = true)]
async fn token_and_wall_budgets() {
    let w = world();
    lists(&w, 5);
    let mut s = spec();
    s.budget.max_tokens = 20;
    let run = w.runtime.start(s).await.unwrap();
    assert_eq!(
        w.runtime.wait(&run).await.unwrap(),
        RunOutcome::BudgetExceeded {
            budget: BudgetKind::Tokens
        }
    );
    let w = world();
    for i in 0..5 {
        w.provider.push_script(
            call(
                &format!("t{i}"),
                "fs_list",
                json!({"path": format!("/d{i}")}),
            )
            .delayed(Duration::from_secs(60)),
        );
    }
    let mut s = spec();
    s.budget.max_wall_ms = 90_000;
    let run = w.runtime.start(s).await.unwrap();
    assert_eq!(
        w.runtime.wait(&run).await.unwrap(),
        RunOutcome::BudgetExceeded {
            budget: BudgetKind::Wall
        }
    );
}

#[tokio::test(start_paused = true)]
async fn loop_detector_stops_repeated_calls() {
    let w = world();
    for i in 0..10 {
        w.provider.push_script(call(
            &format!("t{i}"),
            "fs_read",
            json!({"path": "/Users/ala/a.txt"}),
        ));
    }
    let run = w.runtime.start(spec()).await.unwrap();
    assert_eq!(
        w.runtime.wait(&run).await.unwrap(),
        RunOutcome::LoopDetected {
            tool: "fs_read".into()
        }
    );
    assert_eq!(w.fs.calls(FsToolKind::Read).len(), 2);
    history_is_valid(&w);
}

#[tokio::test(start_paused = true)]
async fn cancel_during_model_stream() {
    let w = world();
    w.provider.push_script(Script::stall());
    let run = w.runtime.start(spec()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    w.runtime.cancel(&run).unwrap();
    let out = tokio::time::timeout(Duration::from_secs(2), w.runtime.wait(&run))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(out, RunOutcome::Cancelled);
}

/// Narzędzie, które czeka do anulowania (jak polecenie powłoki w toku).
struct Hanging(ToolManifest);

#[async_trait]
impl Tool for Hanging {
    fn manifest(&self) -> &ToolManifest {
        &self.0
    }
    async fn call(&self, _: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        ctx.cancel.cancelled().await;
        ToolOutcome::cancelled("długie polecenie")
    }
}

#[tokio::test(start_paused = true)]
async fn cancel_during_tool_stops_at_atomic_point() {
    let mut m = manifest(FsToolKind::Search);
    m.name = "slow_tool".into();
    let w = world_with(RuntimeConfig::default(), vec![Arc::new(Hanging(m))]);
    w.provider.push_script(call("t1", "slow_tool", json!({})));
    w.provider
        .push_script(call("t2", "fs_delete", json!({"path": "/Users/ala/x"})));
    let mut s = spec();
    s.tools.push("slow_tool".into());
    s.roles.clear();
    let run = w.runtime.start(s).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(matches!(
        w.runtime.status(&run).unwrap(),
        RunStatus::Running { .. }
    ));
    w.runtime.cancel(&run).unwrap();
    let out = tokio::time::timeout(Duration::from_secs(2), w.runtime.wait(&run))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(out, RunOutcome::Cancelled);
    assert!(
        w.fs.calls(FsToolKind::Delete).is_empty(),
        "po anulowaniu żaden kolejny krok"
    );
    let events = w.runtime.events(&run).unwrap();
    assert!(events.iter().any(|e| matches!(
        &e.event,
        RunEvent::StepFinished {
            status: agent_runtime_contract::StepStatus::Cancelled,
            ..
        }
    )));
}

#[tokio::test(start_paused = true)]
async fn steering_message_pause_resume_and_goal() {
    let w = world();
    for i in 0..3 {
        w.provider.push_script(
            call(
                &format!("t{i}"),
                "fs_list",
                json!({"path": format!("/d{i}")}),
            )
            .delayed(Duration::from_millis(100)),
        );
    }
    w.provider.push_script(answer("Koniec."));
    let run = w.runtime.start(spec()).await.unwrap();
    w.runtime.steer(&run, Steer::PauseAfterCurrent).unwrap();
    w.runtime
        .steer(&run, Steer::Message("Pomiń pliki .tmp".into()))
        .unwrap();
    let mut sub = w.runtime.subscribe(&run).unwrap();
    loop {
        if matches!(sub.recv().await.unwrap().event, RunEvent::Paused) {
            break;
        }
    }
    assert!(matches!(
        w.runtime.status(&run).unwrap(),
        RunStatus::Paused { .. }
    ));
    let requests_paused = w.provider.requests().len();
    tokio::time::sleep(Duration::from_secs(10)).await;
    assert_eq!(
        w.provider.requests().len(),
        requests_paused,
        "w pauzie żadnej tury"
    );
    w.runtime
        .steer(&run, Steer::ChangeGoal("Uporządkuj tylko PDF-y".into()))
        .unwrap();
    w.runtime.steer(&run, Steer::Resume).unwrap();
    assert!(matches!(
        w.runtime.wait(&run).await.unwrap(),
        RunOutcome::Completed { .. }
    ));
    let next = &w.provider.requests()[requests_paused];
    let texts: Vec<String> = next.messages.iter().map(|m| m.visible_text()).collect();
    assert!(
        texts.iter().any(|t| t.contains("Pomiń pliki .tmp")),
        "{texts:?}"
    );
    assert!(texts.iter().any(|t| t.contains("Uporządkuj tylko PDF-y")));
    let k: Vec<&str> = w
        .runtime
        .events(&run)
        .unwrap()
        .iter()
        .map(|e| e.event.name())
        .collect();
    assert!(k.contains(&"agent.run.steered") && k.contains(&"agent.run.resumed"));
}

#[tokio::test(start_paused = true)]
async fn checkpoint_and_resume_after_restart() {
    let w = world();
    w.provider.push_script(call(
        "t1",
        "fs_list",
        json!({"path": "/Users/ala/Documents"}),
    ));
    w.provider.push_script(Script::stall());
    let run = w.runtime.start(spec()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    // „Awaria”: stan dysku = ostatni checkpoint, wraz z wywołaniem w toku (symulacja restartu w trakcie narzędzia).
    let mut crashed = w.store.latest(&run).unwrap().unwrap();
    assert!(crashed.finished.is_none());
    crashed.pending.push(providers_contract::ToolUse {
        id: "t9".into(),
        name: "fs_move".into(),
        input: json!({}),
    });
    crashed.messages.push(providers_contract::Message::new(
        providers_contract::Role::Assistant,
        vec![ContentBlock::ToolUse(crashed.pending[0].clone())],
    ));
    let tmp = tempfile::tempdir().unwrap();
    let disk = DirCheckpointStore::open(tmp.path()).unwrap();
    disk.save(&crashed).unwrap();
    w.runtime.cancel(&run).unwrap();
    // Nowy proces: ten sam magazyn, nowy dostawca.
    let provider = providers_fake::FakeProvider::new("fake");
    provider.push_script(answer("Dokończone po restarcie."));
    let fs = tools_fs_fake::FakeFsTools::new();
    let rt = Runtime::new(RuntimeDeps {
        provider: Arc::new(provider.clone()),
        tools: fs.tools(),
        checkpoints: Arc::new(disk.clone()),
        bus: None,
        config: RuntimeConfig::default(),
    });
    assert!(rt.status(&run).is_err());
    rt.resume(&run).await.unwrap();
    let out = tokio::time::timeout(Duration::from_secs(5), rt.wait(&run)).await;
    assert!(matches!(
        out.unwrap().unwrap(),
        RunOutcome::Completed { .. }
    ));
    assert!(rt.events(&run).unwrap()[0].event == RunEvent::Resumed);
    assert!(
        fs.calls(FsToolKind::Move).is_empty(),
        "przerwane wywołanie nie jest powtarzane"
    );
    let req = &provider.requests()[0];
    req.validate().unwrap();
    let last = req.messages.last().unwrap();
    let ContentBlock::ToolResult(r) = &last.content[0] else {
        panic!("{last:?}")
    };
    assert!(r.is_error && r.tool_use_id == "t9");
    assert!(matches!(
        rt.resume(&run).await,
        Err(agent_runtime_contract::RunError::AlreadyFinished(_))
    ));
    let none = Runtime::new(RuntimeDeps {
        provider: Arc::new(provider),
        tools: vec![],
        checkpoints: Arc::new(MemCheckpointStore::default()),
        bus: None,
        config: RuntimeConfig::default(),
    });
    assert!(matches!(
        none.resume(&run).await,
        Err(agent_runtime_contract::RunError::NoCheckpoint(_))
    ));
    assert_eq!(w.runtime.wait(&run).await.unwrap(), RunOutcome::Cancelled);
}
