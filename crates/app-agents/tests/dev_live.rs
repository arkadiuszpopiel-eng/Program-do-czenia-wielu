//! Narzędzia programisty na prawdziwym systemie (ręcznie: `cargo test -p app-agents --test
//! dev_live -- --ignored --nocapture`): `git status` w repozytorium Alfy i `cargo test` na małym
//! projekcie tymczasowym z jednym testem zaliczonym i jednym niezaliczonym.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::{DenyLists, PathEnv};
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{Tool, ToolCtx, Toolset};
use tools_dev_impl::{DevTools, DevToolsDeps};
use watchdog_contract::ManualClock;

fn tools(home: &str) -> Vec<Arc<dyn Tool>> {
    let env = PathEnv::windows_profile(home);
    let policy = KernelPolicy::baseline(home, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    broker.script("tools-dev.git", ScriptedDecision::Allow);
    broker.script("tools-dev.test", ScriptedDecision::Allow);
    DevTools::new(DevToolsDeps {
        exec: Arc::new(platform_windows_impl::WindowsPlatform::default()),
        broker,
        env,
        deny: DenyLists::baseline(),
        jobs: None,
        base_env: None,
    })
    .tools()
}

fn ctx(dir: &str) -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta")).with_workdir(dir);
    c.approval_timeout = Duration::from_millis(500);
    c
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "uruchamia prawdziwy git i cargo (kilkanaście sekund)"]
async fn real_git_and_cargo_tests() {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\user".into());
    let tools = tools(&home);
    let git = tools
        .iter()
        .find(|t| t.manifest().name == "dev_git")
        .unwrap();
    let test = tools
        .iter()
        .find(|t| t.manifest().name == "dev_test")
        .unwrap();
    let repo = env!("CARGO_MANIFEST_DIR")
        .replace("/crates/app-agents", "")
        .replace(r"\crates\app-agents", "");
    let status = git.call(json!({"action": "status"}), &ctx(&repo)).await;
    eprintln!("{}", status.text);
    assert!(status.is_ok(), "{}", status.text);
    assert!(status.data["branch"].as_str().is_some());
    let log = git
        .call(json!({"action": "log", "limit": 3}), &ctx(&repo))
        .await;
    assert_eq!(log.text.lines().count(), 3, "{}", log.text);

    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"proba\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "#[test]\nfn dobry() { assert_eq!(2 + 2, 4); }\n#[test]\nfn zly() { assert_eq!(2 + 2, 5); }\n",
    )
    .unwrap();
    let out = test
        .call(json!({}), &ctx(&dir.path().to_string_lossy()))
        .await;
    eprintln!("{}", out.text);
    assert_eq!(out.data["runner"], "cargo");
    assert_eq!(out.data["passed"], 1, "{}", out.text);
    assert_eq!(out.data["failed"], 1);
    assert_eq!(out.data["failures"][0], "zly");
    assert_eq!(out.data["ok"], false);
}
