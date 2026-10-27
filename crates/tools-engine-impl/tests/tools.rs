//! Narzędzia silnika na atrapie portu i Brokerze: kontrakt, cykl pracy agentki (stan → start →
//! transakcja → odczyt → cofnięcie → kroki → zrzut → stop), stały identyfikator transakcji przy
//! ponowieniu, odmowa Brokera, brak katalogu silnika, niezaufana dokumentacja, kill-switch.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::{DenyLists, PathEnv};
use risk_classifier_contract::KernelRule;
use safety_broker_contract::{Holder, KernelPolicy, TaintSource};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{Tool, ToolCtx, ToolStatus, Toolset};
use tools_engine_contract::{EngineMethod, EnginePort, manifests};
use tools_engine_fake::FakeEngine;
use tools_engine_impl::{EngineTools, EngineToolsDeps};
use watchdog_contract::ManualClock;

struct H {
    fake: Arc<FakeEngine>,
    broker: Arc<FakeBroker>,
    tools: EngineTools,
    dir: tempfile::TempDir,
}

fn harness_with(configured: bool) -> H {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_string_lossy().into_owned();
    let env = PathEnv::windows_profile(&home);
    let policy = KernelPolicy::baseline(&home, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    for m in manifests() {
        broker.script(&m.id, ScriptedDecision::Allow);
    }
    let data = dir.path().join("dane");
    let fake = Arc::new(if configured {
        FakeEngine::new(data)
    } else {
        FakeEngine::unconfigured(data)
    });
    let tools = EngineTools::new(EngineToolsDeps {
        port: fake.clone(),
        broker: broker.clone(),
        env,
        deny: DenyLists::baseline(),
    });
    H {
        fake,
        broker,
        tools,
        dir,
    }
}

impl H {
    fn tool(&self, n: &str) -> Arc<dyn Tool> {
        self.tools
            .tools()
            .into_iter()
            .find(|t| t.manifest().name == n)
            .unwrap()
    }

    fn ctx(&self) -> ToolCtx {
        let mut c = ToolCtx::new(Holder::agent("s1", "delta"))
            .with_workdir(&self.dir.path().to_string_lossy());
        c.approval_timeout = Duration::from_millis(200);
        c
    }

    async fn call(&self, n: &str, args: serde_json::Value) -> tools_common_contract::ToolOutcome {
        self.tool(n).call(args, &self.ctx()).await
    }
}

#[tokio::test]
async fn contract_suite() {
    let h = harness_with(true);
    tools_engine_contract::contract_tests::run_all(&h.tools.tools()).await;
}

#[tokio::test]
async fn agent_workflow_on_the_engine() {
    let h = harness_with(true);
    let status = h.call("engine_status", json!({})).await;
    assert!(
        status.is_ok() && status.text.contains("serwer nie działa"),
        "{}",
        status.text
    );

    let early = h.call("engine_inspect", json!({})).await;
    assert!(
        !early.is_ok() && early.text.contains("engine_start"),
        "{}",
        early.text
    );

    let start = h
        .call("engine_start", json!({"scene": "demo", "window": true}))
        .await;
    assert!(start.is_ok(), "{}", start.text);
    assert_eq!(start.data["paused"], true, "okno wymusza zatrzymany czas");
    let again = h.call("engine_start", json!({})).await;
    assert_eq!(
        again.status,
        ToolStatus::Failed {
            error: tools_common_contract::ToolErrorKind::AlreadyExists
        }
    );

    let ops = json!({"operations": [{"op": "entity.spawn", "entity": {"tag": "wieża"}}]});
    let tx = h.call("engine_transaction", ops.clone()).await;
    assert!(tx.is_ok(), "{}", tx.text);
    // Ponowienie tego samego wywołania w tym samym kroku — ten sam identyfikator, bez duplikatu.
    let retry = h.call("engine_transaction", ops).await;
    assert_eq!(retry.data, tx.data);
    assert!(
        tx.data["transaction_id"]
            .as_str()
            .unwrap()
            .starts_with("alfa-")
    );

    let scene = h.call("engine_inspect", json!({"limit": 8})).await;
    assert!(scene.is_ok() && scene.text.contains("wieża"));
    assert_eq!(scene.untrusted, Some(TaintSource::File));
    assert_eq!(scene.data["entity_count"], 2);

    let conflict = h
        .call(
            "engine_transaction",
            json!({"operations": [{"op": "view.configure"}], "expected_revision": 0}),
        )
        .await;
    assert!(
        conflict.text.contains("expected_revision"),
        "{}",
        conflict.text
    );

    let capture = h.call("engine_capture", json!({})).await;
    assert!(capture.is_ok(), "{}", capture.text);
    assert_eq!(capture.images.len(), 1);
    assert_eq!(capture.images[0].media_type, "image/png");

    let undo = h.call("engine_rollback", json!({})).await;
    assert!(undo.is_ok(), "{}", undo.text);
    let step = h.call("engine_step", json!({"frames": 120})).await;
    assert_eq!(step.data["frame"], 120);
    let stream = h.call("engine_stream", json!({})).await;
    assert!(stream.text.contains("wyłączone"), "{}", stream.text);
    let height = h.call("engine_height", json!({"x": 1.0, "z": 2.0})).await;
    assert!(
        !height.is_ok() && height.text.contains("Popraw"),
        "{}",
        height.text
    );
    let hit = h
        .call(
            "engine_raycast",
            json!({"origin": [0, 5, 0], "direction": [0, -1, 0]}),
        )
        .await;
    assert!(hit.text.contains("niczego"));
    let select = h.call("engine_select", json!({"entity_id": 1})).await;
    assert!(select.is_ok(), "{}", select.text);
    let shader = h
        .call(
            "engine_shader",
            json!({"shader_name": "woda", "wgsl_code": "fn main() {}"}),
        )
        .await;
    assert!(shader.is_ok(), "{}", shader.text);

    let stop = h.call("engine_stop", json!({})).await;
    assert_eq!(stop.data["stopped"], true);
    assert!(h.fake.calls().contains(&EngineMethod::HotReloadShader));
}

#[tokio::test]
async fn docs_are_listed_read_in_parts_and_untrusted() {
    let h = harness_with(true);
    let list = h.call("engine_docs", json!({})).await;
    assert!(list.text.contains("docs/API_ETAP_4.md"), "{}", list.text);
    let doc = h
        .call("engine_docs", json!({"file": "docs/API_ETAP_4.md"}))
        .await;
    assert!(doc.text.contains("entity.spawn"));
    assert_eq!(doc.untrusted, Some(TaintSource::File));
    let part = h
        .call(
            "engine_docs",
            json!({"file": "docs/API_ETAP_4.md", "offset": 15}),
        )
        .await;
    assert!(!part.text.starts_with("# API"));
    let missing = h
        .call("engine_docs", json!({"file": "../Cargo.toml"}))
        .await;
    assert!(!missing.is_ok());
}

#[tokio::test]
async fn preview_returns_an_image_without_touching_the_server() {
    let h = harness_with(true);
    let p = h
        .call(
            "engine_preview",
            json!({"frames": 2, "width": 320, "height": 200}),
        )
        .await;
    assert!(p.is_ok(), "{}", p.text);
    assert_eq!(p.images.len(), 1);
    assert_eq!(p.data["telemetry"]["width"], 320);
    assert!(!h.fake.state().running);
}

#[tokio::test]
async fn broker_denial_stops_the_action() {
    let h = harness_with(true);
    h.broker.script(
        "tools-engine.start",
        ScriptedDecision::Deny(KernelRule::SelfEscalation),
    );
    let out = h.call("engine_start", json!({})).await;
    assert!(
        matches!(out.status, ToolStatus::Denied { .. }),
        "{:?}",
        out.status
    );
    assert!(!h.fake.state().running);
}

#[tokio::test]
async fn unconfigured_engine_explains_what_to_do() {
    let h = harness_with(false);
    let status = h.call("engine_status", json!({})).await;
    assert!(
        !status.is_ok() && status.text.contains("Ustawienia"),
        "{}",
        status.text
    );
    let start = h.call("engine_start", json!({})).await;
    assert!(start.text.contains("Ustawienia"), "{}", start.text);
}

#[tokio::test]
async fn kill_switch_ends_the_server_and_cancelled_calls_do_nothing() {
    let h = harness_with(true);
    h.call("engine_start", json!({})).await;
    assert_eq!(h.tools.kill_switch(), 1);
    assert!(!h.fake.state().running);
    let ctx = h.ctx();
    ctx.cancel.cancel();
    let out = h.tool("engine_start").call(json!({}), &ctx).await;
    assert_eq!(out.status, ToolStatus::Cancelled);
    assert!(!h.fake.state().running);
}

#[tokio::test]
async fn relative_world_file_needs_a_workdir_and_is_resolved_against_it() {
    let h = harness_with(true);
    let mut ctx = ToolCtx::new(Holder::agent("s1", "delta"));
    ctx.approval_timeout = Duration::from_millis(200);
    let out = h
        .tool("engine_start")
        .call(json!({"load": "swiat.json"}), &ctx)
        .await;
    assert!(out.text.contains("katalogu roboczego"), "{}", out.text);
    std::fs::write(h.dir.path().join("swiat.json"), "{}").unwrap();
    let ok = h.call("engine_start", json!({"load": "swiat.json"})).await;
    assert!(ok.is_ok(), "{}", ok.text);
}
