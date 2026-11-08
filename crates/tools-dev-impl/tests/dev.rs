//! `tools-dev` na atrapie uruchamiania i Brokerze: kontrakt, wykrycie projektu, podsumowanie
//! testów, stan repozytorium, odmowa Brokera, deny-lista katalogu, brak katalogu roboczego,
//! limit czasu i anulowanie, środowisko bez kluczy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::{DenyLists, PathEnv};
use platform_fake::{FakeExec, FakeRun, command_text};
use risk_classifier_contract::KernelRule;
use safety_broker_contract::{Holder, KernelPolicy, TaintSource};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{Tool, ToolCtx, ToolStatus, Toolset};
use tools_dev_impl::{DevTools, DevToolsDeps};
use watchdog_contract::ManualClock;

struct H {
    exec: Arc<FakeExec>,
    broker: Arc<FakeBroker>,
    tools: DevTools,
    dir: tempfile::TempDir,
}

fn harness() -> H {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_string_lossy().into_owned();
    let env = PathEnv::windows_profile(&home);
    let policy = KernelPolicy::baseline(&home, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    broker.script("tools-dev.git", ScriptedDecision::Allow);
    broker.script("tools-dev.test", ScriptedDecision::Allow);
    let exec = Arc::new(FakeExec::new());
    let tools = DevTools::new(DevToolsDeps {
        exec: exec.clone(),
        broker: broker.clone(),
        env,
        deny: DenyLists::baseline(),
        jobs: None,
        base_env: Some(vec![
            ("PATH".into(), r"C:\Windows".into()),
            ("CARGO_HOME".into(), r"C:\cargo".into()),
            ("OPENAI_API_KEY".into(), "nie-powinno-przejsc".into()),
        ]),
    });
    H {
        exec,
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
}

#[tokio::test]
async fn contract_suite() {
    let h = harness();
    tools_dev_contract::contract_tests::run_all(&h.tools.tools()).await;
}

#[tokio::test]
async fn cargo_tests_detected_run_and_summarised() {
    let h = harness();
    std::fs::write(h.dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
    h.exec.on_command(
        "cargo test",
        FakeRun::exit(
            101,
            "test a::ok ... ok\ntest a::zle ... FAILED\ntest result: FAILED. 7 passed; 1 failed; 0 ignored",
            "",
        ),
    );
    let out = h
        .tool("dev_test")
        .call(json!({"filter": "a::"}), &h.ctx())
        .await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(out.untrusted, Some(TaintSource::File));
    assert_eq!(out.data["runner"], "cargo");
    assert_eq!(out.data["passed"], 7);
    assert_eq!(out.data["failed"], 1);
    assert_eq!(out.data["ok"], false);
    assert!(out.text.starts_with("Testy z błędami") && out.text.contains("a::zle"));
    let spec = &h.exec.runs()[0];
    let cmd = command_text(spec);
    assert!(cmd.contains("cargo test --color never a::"), "{cmd}");
    assert_eq!(spec.process.cwd, h.dir.path());
    let names: Vec<&str> = spec.env.iter().map(|(k, _)| k.as_str()).collect();
    assert!(names.contains(&"CARGO_HOME") && !names.contains(&"OPENAI_API_KEY"));
}

#[tokio::test]
async fn compile_error_timeout_and_unknown_project() {
    let h = harness();
    let none = h.tool("dev_test").call(json!({}), &h.ctx()).await;
    assert!(none.text.contains("Nie rozpoznano"), "{}", none.text);
    h.exec.push(FakeRun::exit(
        101,
        "",
        "error[E0425]: cannot find value `x`",
    ));
    let broken = h
        .tool("dev_test")
        .call(json!({"runner": "cargo"}), &h.ctx())
        .await;
    assert!(broken.text.contains("przed podsumowaniem") && broken.text.contains("E0425"));
    h.exec.push(FakeRun::ok("").taking(5_000));
    let slow = h
        .tool("dev_test")
        .call(json!({"runner": "go", "timeout_s": 1}), &h.ctx())
        .await;
    assert_eq!(slow.data["timed_out"], true, "{}", slow.text);
}

#[tokio::test]
async fn git_status_parsed_and_broker_required() {
    let h = harness();
    h.exec.on_command(
        "status --porcelain",
        FakeRun::ok("## main...origin/main [ahead 1]\n M src/lib.rs\n?? nowy.txt\n"),
    );
    let out = h
        .tool("dev_git")
        .call(json!({"action": "status"}), &h.ctx())
        .await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(out.data["branch"], "main");
    assert_eq!(out.data["ahead"], 1);
    assert_eq!(out.data["files"].as_array().unwrap().len(), 2);
    h.exec
        .push(FakeRun::exit(128, "", "fatal: not a git repository"));
    let not_repo = h
        .tool("dev_git")
        .call(json!({"action": "log"}), &h.ctx())
        .await;
    assert!(not_repo.text.contains("nie jest repozytorium"));
    h.broker.script(
        "tools-dev.git",
        ScriptedDecision::Deny(KernelRule::CredentialDenylist),
    );
    let runs = h.exec.runs().len();
    let denied = h
        .tool("dev_git")
        .call(json!({"action": "diff"}), &h.ctx())
        .await;
    assert!(matches!(denied.status, ToolStatus::Denied { .. }));
    assert_eq!(h.exec.runs().len(), runs, "bez zgody — bez uruchomienia");
}

#[tokio::test]
async fn workdir_required_and_deny_list() {
    let h = harness();
    let no_dir = ToolCtx::new(Holder::agent("s1", "delta"));
    let out = h
        .tool("dev_git")
        .call(json!({"action": "status"}), &no_dir)
        .await;
    assert!(out.text.contains("katalogu roboczego"), "{}", out.text);
    let ssh = h.dir.path().join(".ssh");
    std::fs::create_dir_all(&ssh).unwrap();
    let ctx = ToolCtx::new(Holder::agent("s1", "delta")).with_workdir(&ssh.to_string_lossy());
    let out = h
        .tool("dev_git")
        .call(json!({"action": "status"}), &ctx)
        .await;
    assert!(matches!(out.status, ToolStatus::Denied { .. }), "{out:?}");
    assert!(h.exec.runs().is_empty());
}
