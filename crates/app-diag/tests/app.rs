//! Diagnostyka w aplikacji na atrapie sondy: stan, zapis, testy z kluczy, raport w pliku,
//! narzędzie agentek `system_diagnostics` przez Brokera (testy dysków niedostępne dla agentki).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use app_diag::{DiagApp, DiagTool};
use pc_diag_contract::{CheckKind, Verdict};
use pc_diag_fake::FakeProbe;
use pc_diag_impl::{PcDiagDeps, PcDiagService};
use risk_classifier_contract::KernelRule;
use safety_broker_contract::{Holder, TaintSource};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{BrokerGate, Tool, ToolCtx, ToolStatus};

fn app(dir: &std::path::Path) -> Arc<DiagApp> {
    let mut deps = PcDiagDeps::new(
        Arc::new(FakeProbe::default()),
        dir.join("history"),
        dir.join("work"),
    );
    deps.disk_sample_mb = 16;
    deps.cpu_window = Duration::from_millis(50);
    Arc::new(DiagApp::with_service(
        PcDiagService::new(deps),
        dir.join("Diagnostyka"),
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn state_snapshot_run_export() {
    let dir = tempfile::tempdir().unwrap();
    let a = app(dir.path());
    let empty = a.state();
    assert!(empty.latest.is_none() && empty.integrity.is_none() && empty.running.is_none());
    let v = a.snapshot().await.unwrap();
    assert_eq!(v.snapshots, 1);
    assert_eq!(v.integrity.as_ref().unwrap().verdict, Verdict::Warn);
    assert!(v.available.contains(&"disk:E:".to_owned()));
    assert!(a.run(vec!["zly".into()]).await.is_err());
    assert!(a.run(Vec::new()).await.is_err());
    let run = a
        .run(vec!["cpu_single".into(), "memory".into(), "disk:C:".into()])
        .await
        .unwrap();
    assert_eq!(run.results.len(), 3);
    assert!(a.state().running.is_none(), "postęp wyczyszczony po końcu");
    assert_eq!(a.state().runs, 1);
    a.run(vec!["cpu_single".into()]).await.unwrap();
    let runs = a.runs(10);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].results.len(), 1, "najnowszy pierwszy");
    let path = a.export().await.unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# Diagnostyka komputera") && text.contains("## Testy"));
    assert!(path.contains("Diagnostyka") && path.ends_with(".md"));
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_millis(300);
    c
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_tool_reports_and_respects_broker() {
    let dir = tempfile::tempdir().unwrap();
    let a = app(dir.path());
    let broker = Arc::new(FakeBroker::new().unwrap());
    broker.script("app-diag.report", ScriptedDecision::Allow);
    let tool = DiagTool::new(a.clone(), BrokerGate::new(broker.clone()));
    tools_common_contract::contract_tests::manifest_is_valid(&tool);
    // Pod tą nazwą narzędzie zna model (katalog funkcji, docs/features/narzedzia.txt).
    assert_eq!(tool.manifest().name, "system_diagnostics");
    tools_common_contract::contract_tests::invalid_args_rejected(&tool, "C:\\").await;
    let out = tool.call(json!({}), &ctx()).await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(out.untrusted, Some(TaintSource::File));
    assert!(out.text.starts_with("Ocena: UWAGA"), "{}", &out.text[..80]);
    assert!(out.text.contains("Dysk USB 2TB"));
    let with_tests = tool
        .call(json!({"tests": ["cpu_single", "network"]}), &ctx())
        .await;
    assert!(with_tests.is_ok());
    assert_eq!(
        with_tests.data["run"]["results"].as_array().unwrap().len(),
        2
    );
    let disk = tool.call(json!({"tests": ["disk:C:"]}), &ctx()).await;
    assert!(matches!(disk.status, ToolStatus::Failed { .. }));
    assert!(disk.text.contains("niedostępny dla agentki"));
    broker.script(
        "app-diag.report",
        ScriptedDecision::Deny(KernelRule::GuiControlOfKernelProcess),
    );
    let denied = tool.call(json!({"refresh": true}), &ctx()).await;
    assert!(matches!(denied.status, ToolStatus::Denied { .. }));
    assert_eq!(a.service().history().runs().len(), 1, "odmowa — bez testów");
    let _ = CheckKind::Integrity;
}
