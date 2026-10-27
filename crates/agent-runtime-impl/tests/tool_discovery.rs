//! Dobór małego katalogu, wyszukiwanie i krótkie komendy bez poszerzania uprawnień.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use agent_runtime_contract::{AgentRuntime, CheckpointStore};
use agent_runtime_impl::RuntimeConfig;
use common::{answer, call, spec, world_with};
use serde_json::json;
use std::sync::Arc;
use tools_common_contract::{ScriptedTool, Tool, ToolOutcome, Toolset};
use tools_fs_contract::{FsToolKind, manifest};

fn catalog() -> Vec<Arc<dyn Tool>> {
    (0..80)
        .map(|i| {
            let mut m = manifest(FsToolKind::Read);
            m.name = format!("device_status_{i}");
            m.title = format!("Stan urządzenia {i}");
            m.description = format!(
                "Odczytuje stan urządzenia {i}; nie modyfikuje konfiguracji ani danych użytkownika."
            );
            m.input_schema = json!({"type":"object","properties":{},"additionalProperties":false});
            Arc::new(ScriptedTool::new(m)) as Arc<dyn Tool>
        })
        .collect()
}
fn setup() -> (common::World, agent_runtime_contract::RunSpec) {
    let extra = catalog();
    let mut s = spec();
    s.goal = "Read README.md and inspect project files".into();
    s.verify = false;
    s.tools
        .extend(extra.iter().map(|t| t.manifest().name.clone()));
    (world_with(RuntimeConfig::default(), extra), s)
}

#[tokio::test(start_paused = true)]
async fn large_catalog_starts_small_and_offers_discovery_and_short_commands() {
    let (w, s) = setup();
    w.provider.push_script(answer("Gotowe."));
    let run = w.runtime.start(s).await.unwrap();
    w.runtime.wait(&run).await.unwrap();
    let req = w.provider.requests().remove(0);
    assert!(req.tools.len() <= 24, "{} definicji", req.tools.len());
    assert!(req.tools.iter().any(|t| t.name == "tools_find"));
    assert!(req.tools.iter().any(|t| t.name == "tool_call"));
}

#[tokio::test(start_paused = true)]
async fn exact_discovery_activates_schema_and_survives_in_checkpoint() {
    let (w, s) = setup();
    w.provider.push_script(call(
        "find",
        "tools_find",
        json!({"query":"device_status_79"}),
    ));
    w.provider.push_script(answer("Mam właściwe narzędzie."));
    let run = w.runtime.start(s).await.unwrap();
    w.runtime.wait(&run).await.unwrap();
    let requests = w.provider.requests();
    assert!(
        requests[1]
            .tools
            .iter()
            .any(|t| t.name == "device_status_79")
    );
    let cp = w.store.latest(&run).unwrap().unwrap();
    let value = serde_json::to_value(cp).unwrap();
    assert!(
        value["discovered_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n == "device_status_79")
    );
}

#[tokio::test(start_paused = true)]
async fn short_read_uses_canonical_tool_and_records_it() {
    let (w, s) = setup();
    w.fs.set_default(FsToolKind::Read, ToolOutcome::ok("read-ok", json!({})));
    w.provider.push_script(call(
        "read",
        "tool_call",
        json!({"command":"read","args":{"path":"/Users/ala/Documents/README.md"}}),
    ));
    w.provider.push_script(answer("Odczytano."));
    let run = w.runtime.start(s).await.unwrap();
    w.runtime.wait(&run).await.unwrap();
    assert_eq!(w.fs.calls(FsToolKind::Read).len(), 1);
    let events = w.runtime.events(&run).unwrap();
    assert!(events.iter().any(|e| matches!(&e.event, agent_runtime_contract::RunEvent::StepFinished {tool:Some(t),..} if t=="fs_read")));
}

#[tokio::test(start_paused = true)]
async fn short_command_cannot_call_a_tool_outside_run_scope() {
    let (w, mut s) = setup();
    s.tools.retain(|t| t != "fs_write");
    w.provider.push_script(call("write", "tool_call", json!({"command":"write","args":{"path":"/Users/ala/Documents/x","content":"x","mode":"create"}})));
    w.provider.push_script(answer("Brak dostępu."));
    let run = w.runtime.start(s).await.unwrap();
    w.runtime.wait(&run).await.unwrap();
    assert!(w.fs.calls(FsToolKind::Write).is_empty());
    let requests = w.provider.requests();
    assert!(!requests[0].tools.iter().any(|t| t.name == "fs_write"));
}

#[tokio::test(start_paused = true)]
async fn short_and_canonical_names_share_one_loop_counter() {
    let (w, s) = setup();
    let args = json!({"path":"/Users/ala/Documents/x"});
    w.provider.push_script(call(
        "a",
        "tool_call",
        json!({"command":"read","args":args}),
    ));
    w.provider.push_script(call("b", "fs_read", args.clone()));
    w.provider.push_script(call(
        "c",
        "tool_call",
        json!({"command":"czytaj","args":args}),
    ));
    let run = w.runtime.start(s).await.unwrap();
    let outcome = w.runtime.wait(&run).await.unwrap();
    assert!(
        matches!(outcome, agent_runtime_contract::RunOutcome::LoopDetected {tool} if tool=="fs_read")
    );
    assert_eq!(w.fs.calls(FsToolKind::Read).len(), 2);
}

#[tokio::test(start_paused = true)]
async fn verification_rejects_mutation_through_short_command() {
    let (w, mut s) = setup();
    s.verify = true;
    w.provider.push_script(answer("Wykonano."));
    w.provider.push_script(call("write", "tool_call", json!({"command":"write","args":{"path":"/Users/ala/Documents/x","content":"x","mode":"create"}})));
    w.provider.push_script(answer("WERYFIKACJA: OK"));
    let run = w.runtime.start(s).await.unwrap();
    w.runtime.wait(&run).await.unwrap();
    assert!(w.fs.calls(FsToolKind::Write).is_empty());
}

#[tokio::test(start_paused = true)]
#[allow(clippy::print_stdout)] // Jawny wynik porównania wielkości pakietu definicji.
async fn large_catalog_payload_is_measurably_smaller() {
    let (w, s) = setup();
    let all =
        w.fs.tools()
            .into_iter()
            .chain(catalog())
            .filter(|t| s.tools.contains(&t.manifest().name))
            .map(|t| t.manifest().to_spec())
            .collect::<Vec<_>>();
    let baseline = serde_json::to_vec(&all).unwrap().len();
    w.provider.push_script(answer("Gotowe."));
    let run = w.runtime.start(s).await.unwrap();
    w.runtime.wait(&run).await.unwrap();
    let request = w.provider.requests().remove(0);
    let reduced = serde_json::to_vec(&request.tools).unwrap().len();
    let measured = json!({"dataset":"synthetic_86_tools","baseline_tools":all.len(),"selected_tools":request.tools.len(),"baseline_schema_bytes":baseline,"selected_schema_bytes":reduced,"reduction_percent":100.0*(1.0-reduced as f64/baseline as f64)});
    println!("TOOL_CATALOG_BENCHMARK {measured}");
    assert!(reduced * 100 < baseline * 60, "{measured}");
}

#[tokio::test(start_paused = true)]
async fn repeated_invalid_short_commands_stop_without_burning_the_budget() {
    let (w, mut s) = setup();
    s.tools.retain(|n| n != "fs_write");
    for i in 0..3 {
        w.provider.push_script(call(&format!("bad{i}"),"tool_call",json!({"command":"write","args":{"path":"/Users/ala/Documents/x","content":"x","mode":"create"}})));
    }
    let run = w.runtime.start(s).await.unwrap();
    assert!(matches!(
        w.runtime.wait(&run).await.unwrap(),
        agent_runtime_contract::RunOutcome::LoopDetected { .. }
    ));
    assert!(w.fs.calls(FsToolKind::Write).is_empty());
}

#[tokio::test(start_paused = true)]
async fn restarted_runtime_restores_discovered_schema_from_disk() {
    let (w, s) = setup();
    let id = agent_runtime_contract::RunId::new("catalog-resume");
    let mut cp = agent_runtime_contract::Checkpoint::initial(id.clone(), s);
    cp.discovered_tools.push("device_status_79".into());
    let dir = tempfile::tempdir().unwrap();
    let disk = agent_runtime_impl::DirCheckpointStore::open(dir.path()).unwrap();
    disk.save(&cp).unwrap();
    w.provider.push_script(answer("Wznowiono."));
    let rt = agent_runtime_impl::Runtime::new(agent_runtime_impl::RuntimeDeps {
        provider: Arc::new(w.provider.clone()),
        tools: w.fs.tools().into_iter().chain(catalog()).collect(),
        checkpoints: Arc::new(disk),
        bus: None,
        config: RuntimeConfig::default(),
    });
    rt.resume(&id).await.unwrap();
    rt.wait(&id).await.unwrap();
    assert!(
        w.provider.requests()[0]
            .tools
            .iter()
            .any(|t| t.name == "device_status_79")
    );
    let mut old = serde_json::to_value(cp).unwrap();
    old.as_object_mut().unwrap().remove("discovered_tools");
    let legacy: agent_runtime_contract::Checkpoint = serde_json::from_value(old).unwrap();
    assert!(legacy.discovered_tools.is_empty());
}

#[tokio::test(start_paused = true)]
async fn readonly_role_filters_both_discovery_and_short_execution() {
    let (w, mut s) = setup();
    s.roles = personas_contract::builtin_roles()
        .into_iter()
        .filter(|r| r.id.as_str() == "critic")
        .collect();
    w.provider
        .push_script(call("find", "tools_find", json!({"query":"write"})));
    w.provider.push_script(call("write","tool_call",json!({"command":"write","args":{"path":"/Users/ala/Documents/x","content":"x","mode":"create"}})));
    w.provider.push_script(answer("Tylko odczyt."));
    let run = w.runtime.start(s).await.unwrap();
    w.runtime.wait(&run).await.unwrap();
    assert!(w.fs.calls(FsToolKind::Write).is_empty());
    assert!(w.provider.requests().iter().all(|r| {
        r.tools
            .iter()
            .all(|t| t.name != "fs_write" && t.name != "fs_delete")
    }));
}
