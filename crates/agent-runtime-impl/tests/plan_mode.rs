//! Tryb „Plan” (plan 50 §4.1, Z.5): rozmowa na L0 — agentka dostaje tylko narzędzia do odczytu
//! i zasadę planu w prompcie, wywołanie zmieniające jest odrzucane bez wykonania, a odpowiedź
//! (plan) kończy przebieg bez samoweryfikacji. Na innym poziomie — zwykły przebieg.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use agent_runtime_contract::{AgentRuntime, MemCheckpointStore, RunOutcome};
use agent_runtime_impl::{AutonomyOracle, Runtime, RuntimeConfig, RuntimeDeps, RuntimeExt};
use common::{answer, call, spec};
use core_bus_contract::{AgentId, SessionId};
use providers_fake::FakeProvider;
use risk_classifier_contract::AutonomyLevel;
use serde_json::json;
use tools_common_contract::Toolset;
use tools_fs_contract::FsToolKind;
use tools_fs_fake::FakeFsTools;

struct Level(AutonomyLevel);

impl AutonomyOracle for Level {
    fn level(&self, _session: &SessionId, _agent: &AgentId) -> AutonomyLevel {
        self.0
    }
}

fn runtime(level: AutonomyLevel) -> (FakeProvider, FakeFsTools, Runtime) {
    let provider = FakeProvider::new("fake");
    let fs = FakeFsTools::new();
    let runtime = Runtime::with_ext(
        RuntimeDeps {
            provider: Arc::new(provider.clone()),
            tools: fs.tools(),
            checkpoints: Arc::new(MemCheckpointStore::default()),
            bus: None,
            config: RuntimeConfig::default(),
        },
        RuntimeExt {
            locks: None,
            autonomy: Some(Arc::new(Level(level))),
        },
    );
    (provider, fs, runtime)
}

#[tokio::test(start_paused = true)]
async fn plan_mode_reads_only_and_ends_with_the_plan() {
    let (provider, fs, runtime) = runtime(AutonomyLevel::L0);
    provider.push_script(call(
        "w1",
        "fs_write",
        json!({"path": "notatka.txt", "content": "x"}),
    ));
    provider.push_script(answer(
        "Plan: 1. przeczytam pliki, 2. uporządkuję. Wykonać?",
    ));
    let mut s = spec();
    s.verify = true;
    let run = runtime.start(s).await.unwrap();
    let outcome = runtime.wait(&run).await.unwrap();
    assert!(
        matches!(&outcome, RunOutcome::Completed { summary, verified: None } if summary.starts_with("Plan:")),
        "{outcome:?}"
    );
    assert!(
        fs.calls(FsToolKind::Write).is_empty(),
        "zapis w trybie Plan"
    );
    let requests = provider.requests();
    assert_eq!(requests.len(), 2, "bez tury weryfikacji");
    let first = &requests[0];
    assert!(first.system.as_deref().unwrap_or("").contains("Tryb Plan"));
    let names: Vec<_> = first.tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"fs_read"), "{names:?}");
    assert!(!names.contains(&"fs_write"), "{names:?}");
    let rejected = format!("{:?}", requests[1].messages);
    assert!(rejected.contains("Tryb Plan: tylko odczyt"), "{rejected}");
}

#[tokio::test(start_paused = true)]
async fn other_levels_keep_writing_tools_and_no_plan_note() {
    for level in [AutonomyLevel::L1, AutonomyLevel::L3] {
        let (provider, _fs, runtime) = runtime(level);
        provider.push_script(answer("Gotowe."));
        let run = runtime.start(spec()).await.unwrap();
        runtime.wait(&run).await.unwrap();
        let first = provider.requests().remove(0);
        assert!(!first.system.as_deref().unwrap_or("").contains("Tryb Plan"));
        assert!(
            first.tools.iter().any(|t| t.name == "fs_write"),
            "{level:?}"
        );
    }
}
