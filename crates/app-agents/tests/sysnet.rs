//! F6 `tools-system` i `tools-net` w rejestrze agentek (atrapy: `FakeSys`, `FakeHttp`,
//! `FakeDownloads`; Broker na prawdziwym silniku): narzędzia obecne, przydział rolami wg zasady
//! najmniejszych uprawnień (Wykonawczyni — system i sieć, Badaczka — tylko odczyt sieci, reszta
//! bez), zakończenie procesu Alfy odrzucone w rejestrze aplikacji, cofanie zapisu zmiennej
//! przez `AgentTools::undo_env`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use app_agents::{AgentTools, AppsDeps, EnvUndoError, ShellToolsConfig, SysNetDeps, ToolsDeps};
use compliance_contract::{DenyLists, PathEnv};
use personas_contract::{Role, builtin_roles};
use platform_apps_contract::{BrowserKind, BrowserSpec, EnvScope};
use platform_apps_fake::{FakeBrowser, FakeDownloads, FakeOffice, FakeSys, SysCall, fake_process};
use platform_contract::{FsPort, TargetGuard};
use platform_fake::{FakeExec, FakeFs};
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{Tool, ToolCtx, ToolStatus};
use tools_net_fake::FakeHttp;
use undo_journal_contract::UndoLimits;
use undo_journal_fake::FakeUndoJournal;
use watchdog_contract::ManualClock;

fn abs(rel: &str) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" });
    rel.split('/')
        .filter(|s| !s.is_empty())
        .fold(root, |acc, seg| acc.join(seg))
}

struct H {
    tools: AgentTools,
    sys: Arc<FakeSys>,
    net: Arc<FakeHttp>,
}

fn harness() -> H {
    let home = abs("Users/ala").to_string_lossy().into_owned();
    let env = PathEnv::windows_profile(&home);
    let policy = KernelPolicy::baseline(&home, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    for t in [
        "tools-system.process_kill",
        "tools-system.env_set",
        "tools-net.fetch",
    ] {
        broker.script(t, ScriptedDecision::Allow);
    }
    let fs = Arc::new(FakeFs::default());
    let port: Arc<dyn FsPort> = fs.clone();
    let journal =
        FakeUndoJournal::new(port, UndoLimits::default(), Arc::new(|| 1_000_000u64)).unwrap();
    let local = abs("Users/ala/AppData/Local/Alfa");
    let sys = Arc::new(FakeSys::new(app_agents::sysnet_guard(&local)));
    sys.add_process(fake_process(std::process::id(), 1, "alfa.exe"));
    sys.add_process(fake_process(5000, std::process::id(), "msedgewebview2.exe"));
    sys.add_process(fake_process(4321, 1, "notepad.exe"));
    let net = Arc::new(FakeHttp::default());
    net.page("https://example.com/", "text/plain", b"ok");
    let apps = AppsDeps {
        office: Arc::new(FakeOffice::new()),
        browser: Arc::new(FakeBrowser::new()),
        browser_spec: BrowserSpec {
            kind: BrowserKind::Edge,
            executable: None,
            alfa_root: local.clone(),
            profile_dir: local.join("browser").join("profile"),
            quarantine_dir: local.join("browser").join("quarantine"),
            headless: true,
        },
        plugins_dir: None,
        sysnet: Some(SysNetDeps {
            sys: sys.clone(),
            power: None,
            desktop: None,
            hardware: None,
            http: net.clone(),
            search: None,
            downloads: Arc::new(FakeDownloads::default()),
            quarantine_root: None,
        }),
    };
    let tools = AgentTools::new(ToolsDeps {
        broker,
        journal: Arc::new(journal),
        fs,
        exec: Arc::new(FakeExec::new()),
        clipboard: None,
        env,
        deny: DenyLists::baseline(),
        jobs: None,
        bus: None,
        shell: ShellToolsConfig::default(),
        base_env: None,
        extra: Vec::new(),
        apps: Some(apps),
    });
    H { tools, sys, net }
}

fn role(id: &str) -> Role {
    builtin_roles()
        .into_iter()
        .find(|r| r.id.as_str() == id)
        .unwrap()
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_millis(200);
    c
}

fn tool(h: &H, name: &str) -> Arc<dyn Tool> {
    h.tools
        .all()
        .into_iter()
        .find(|t| t.manifest().name == name)
        .unwrap()
}

#[test]
fn registered_and_routed_by_least_privilege() {
    let h = harness();
    let names = h.tools.names();
    for n in [
        "system_processes",
        "system_process_kill",
        "system_service_control",
        "system_env_set",
        "system_status",
        "net_fetch",
        "net_download",
    ] {
        assert!(names.contains(&n.to_owned()), "{n}");
    }
    assert!(!names.contains(&"net_search".to_owned()), "bez dostawcy");
    let allowed = |r: &str| h.tools.allowed_for(&[role(r)]);
    let operator = allowed("operator");
    assert!(operator.iter().any(|n| n == "system_process_kill"));
    assert!(operator.iter().any(|n| n == "net_download"));
    let researcher = allowed("researcher");
    assert!(researcher.contains(&"net_fetch".to_owned()));
    assert!(!researcher.contains(&"net_download".to_owned()));
    assert!(!researcher.iter().any(|n| n.starts_with("system_")));
    for r in [
        "critic",
        "keeper",
        "conductor",
        "speaker",
        "thinker",
        "writer",
        "coder",
    ] {
        assert!(
            !allowed(r)
                .iter()
                .any(|n| n.starts_with("system_") || n.starts_with("net_")),
            "{r}"
        );
    }
    let guard = app_agents::sysnet_guard(&abs("x"));
    assert!(guard.is_protected(std::process::id(), "cokolwiek.exe"));
    assert!(TargetGuard::baseline().is_protected(1, "alfa-watchdog.exe"));
}

#[tokio::test]
async fn alfa_process_tree_cannot_be_killed_through_the_registry() {
    let h = harness();
    for (pid, name) in [
        (std::process::id(), "alfa.exe"),
        (5000, "msedgewebview2.exe"),
    ] {
        let out = tool(&h, "system_process_kill")
            .call(json!({"pid": pid, "name": name}), &ctx())
            .await;
        assert!(matches!(out.status, ToolStatus::Denied { .. }), "{name}");
    }
    let ok = tool(&h, "system_process_kill")
        .call(json!({"pid": 4321, "name": "notepad.exe"}), &ctx())
        .await;
    assert!(ok.is_ok(), "{}", ok.text);
    assert_eq!(h.sys.calls(), vec![SysCall::Terminated(4321)]);
}

#[tokio::test]
async fn env_undo_and_fetch_through_app_registry() {
    let h = harness();
    h.sys.set_var(EnvScope::User, "EDITOR", "notepad");
    let set = tool(&h, "system_env_set")
        .call(json!({"name": "EDITOR", "value": "code"}), &ctx())
        .await;
    assert!(set.is_ok(), "{}", set.text);
    let id = set.data["undo_id"].as_u64().unwrap();
    assert!(h.tools.undo_env(id).unwrap().contains("EDITOR"));
    assert_eq!(h.tools.undo_env(id), Err(EnvUndoError::Unknown(id)));
    let out = tool(&h, "net_fetch")
        .call(json!({"url": "https://example.com/"}), &ctx())
        .await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(h.net.requests().len(), 1);
}
