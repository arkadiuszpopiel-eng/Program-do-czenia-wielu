//! Narzędzia F6/F8 w rejestrze agentek (atrapy: `platform-apps-fake`, `tools-office-fake`,
//! `tools-browser-fake`, Broker na prawdziwym silniku): Office i przeglądarka obecne, przydział
//! rolami wg zasady najmniejszych uprawnień (Krytyczka tylko odczyt dokumentów), kill-switch
//! zamyka przeglądarki agentek, narzędzia wtyczek pojawiają się i znikają bez przebudowy rejestru.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use app_agents::{AgentTools, AppsDeps, ShellToolsConfig, ToolsDeps};
use compliance_contract::{DenyLists, PathEnv};
use personas_contract::{Role, builtin_roles};
use platform_apps_contract::{BrowserKind, BrowserSpec};
use platform_apps_fake::{FakeBrowser, FakeDocument, FakeOffice, FakePage};
use platform_contract::FsPort;
use platform_fake::{FakeExec, FakeFs};
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{Tool, ToolCtx, Toolset};
use undo_journal_contract::UndoLimits;
use undo_journal_fake::FakeUndoJournal;
use watchdog_contract::ManualClock;

const HOME: &str = "/Users/ala";
const DOC: &str = "/Users/ala/Documents/Raport.docx";
const ALFA: &str = "Users/ala/AppData/Local/Alfa";

/// Ścieżka bezwzględna na bieżącym systemie (na Windows `/Users/…` nie jest bezwzględna, a
/// `BrowserSpec::validate` słusznie odrzuca katalog Alfy, który nie jest bezwzględny).
fn abs(rel: &str) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" });
    rel.split('/')
        .filter(|s| !s.is_empty())
        .fold(root, |acc, seg| acc.join(seg))
}

struct H {
    tools: AgentTools,
    web: Arc<FakeBrowser>,
    office: Arc<FakeOffice>,
    _dir: tempfile::TempDir,
}

fn harness(extra: Vec<Arc<dyn Tool>>, apps: bool) -> H {
    let doc = FakeDocument::word(&["Raport kwartalny", "Kwota: 100 zł"]);
    let fs = Arc::new(FakeFs::with_files([(PathBuf::from(DOC), doc.to_bytes())]));
    let env = PathEnv::windows_profile(HOME);
    let policy = KernelPolicy::baseline(HOME, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    for t in ["tools-office.read", "tools-browser.open"] {
        broker.script(t, ScriptedDecision::Allow);
    }
    let port: Arc<dyn FsPort> = fs.clone();
    let journal =
        FakeUndoJournal::new(port, UndoLimits::default(), Arc::new(|| 1_000_000u64)).unwrap();
    let web = Arc::new(FakeBrowser::new());
    web.add_page(
        "https://sklep.pl/",
        FakePage {
            title: "Sklep".into(),
            ..FakePage::default()
        },
    );
    let office = Arc::new(FakeOffice::new());
    let dir = tempfile::tempdir().unwrap();
    let apps = apps.then(|| AppsDeps {
        office: office.clone(),
        browser: web.clone(),
        browser_spec: BrowserSpec {
            kind: BrowserKind::Edge,
            executable: None,
            alfa_root: abs(ALFA),
            profile_dir: abs(ALFA).join("browser").join("profile"),
            quarantine_dir: abs(ALFA).join("browser").join("quarantine"),
            headless: true,
        },
        plugins_dir: Some(dir.path().join("plugins")),
    });
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
        extra,
        apps,
    });
    H {
        tools,
        web,
        office,
        _dir: dir,
    }
}

fn role(id: &str) -> Role {
    builtin_roles()
        .into_iter()
        .find(|r| r.id.as_str() == id)
        .unwrap()
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta")).with_workdir("/Users/ala/Documents");
    c.approval_timeout = Duration::from_millis(300);
    c
}

fn call(h: &H, name: &str) -> Arc<dyn Tool> {
    h.tools
        .all()
        .into_iter()
        .find(|t| t.manifest().name == name)
        .unwrap()
}

#[test]
fn office_and_browser_are_registered() {
    let h = harness(Vec::new(), true);
    let names = h.tools.names();
    for n in [
        "office_read",
        "office_edit",
        "browser_open",
        "browser_read",
        "browser_close",
    ] {
        assert!(names.contains(&n.to_owned()), "{n}: {names:?}");
    }
    assert!(h.tools.manifest("office_edit").unwrap().mutating);
    let without = harness(Vec::new(), false);
    assert!(
        !without
            .tools
            .names()
            .iter()
            .any(|n| n.starts_with("office_"))
    );
}

#[test]
fn roles_get_least_privilege() {
    let h = harness(Vec::new(), true);
    let has = |r: &str, t: &str| h.tools.allowed_for(&[role(r)]).contains(&t.to_owned());
    // Wykonawczyni: Office, przeglądarka (i wtyczki — grupa `plugin`).
    assert!(has("operator", "office_edit") && has("operator", "browser_open"));
    assert!(role("operator").tools.contains(&"plugin".to_owned()));
    // Badaczka: przeglądarka, bez dokumentów.
    assert!(has("researcher", "browser_open") && !has("researcher", "office_read"));
    // Krytyczka: tylko odczyt dokumentów, bez przeglądarki i bez edycji.
    assert!(has("critic", "office_read"));
    assert!(!has("critic", "office_edit") && !has("critic", "browser_read"));
    // Pisarka: dokumenty, bez przeglądarki.
    assert!(has("writer", "office_edit") && !has("writer", "browser_open"));
    // Strażniczka, Dyrygentka, Mówczyni, Myślicielka: żadnych aplikacji.
    for r in ["keeper", "conductor", "speaker", "thinker"] {
        let allowed = h.tools.allowed_for(&[role(r)]);
        assert!(
            !allowed
                .iter()
                .any(|n| n.starts_with("office_") || n.starts_with("browser_")),
            "{r}: {allowed:?}"
        );
    }
}

#[test]
fn fake_toolsets_route_by_role_groups() {
    let mut extra = tools_office_fake::FakeTools::default().tools();
    extra.extend(tools_browser_fake::FakeTools::default().tools());
    let h = harness(extra, false);
    let critic = h.tools.allowed_for(&[role("critic")]);
    assert!(critic.contains(&"office_read".to_owned()));
    assert!(
        !critic
            .iter()
            .any(|n| n == "office_edit" || n.starts_with("browser_"))
    );
    let researcher = h.tools.allowed_for(&[role("researcher")]);
    assert!(researcher.contains(&"browser_open".to_owned()));
}

#[tokio::test]
async fn office_read_goes_through_fake_office() {
    let h = harness(Vec::new(), true);
    let out = call(&h, "office_read")
        .call(json!({"path": "Raport.docx"}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert!(out.text.contains("Kwota: 100 zł"));
    assert!(out.untrusted.is_some());
    assert_eq!(h.office.macros_run(), 0);
}

#[tokio::test]
async fn kill_switch_closes_agent_browsers() {
    let h = harness(Vec::new(), true);
    let out = call(&h, "browser_open")
        .call(json!({"url": "https://sklep.pl/"}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(h.web.open_sessions(), 1);
    assert_eq!(h.tools.kill_switch(), 1);
    assert_eq!(h.web.open_sessions(), 0, "przeglądarka zamknięta");
    let after = call(&h, "browser_read").call(json!({}), &ctx()).await;
    assert!(!after.is_ok(), "po kill-switchu brak sesji przeglądarki");
    assert_eq!(h.tools.kill_switch(), 0);
    assert_eq!(harness(Vec::new(), false).tools.kill_switch(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn plugin_tools_follow_state_without_rebuilding() {
    let h = harness(Vec::new(), true);
    let plugins = h.tools.plugins();
    let before = h.tools.names().len();
    assert!(plugins.list().unwrap().plugins.is_empty());
    let wasm = wat::parse_str(PLUGIN).unwrap();
    let manifest = plugin_runtime_contract::samples::manifest("licznik", "1.0.0", &wasm);
    let b64 = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(&wasm)
    };
    let card = plugins
        .propose(serde_json::to_value(manifest).unwrap(), &b64)
        .await
        .unwrap();
    assert_eq!(h.tools.names().len(), before, "propozycja — bez narzędzi");
    plugins
        .approve("licznik", "1.0.0", &card.review_hash)
        .await
        .unwrap();
    assert!(h.tools.names().contains(&"plugin_word_count".to_owned()));
    assert!(
        h.tools
            .allowed_for(&[role("operator")])
            .contains(&"plugin_word_count".to_owned())
    );
    assert!(
        !h.tools
            .allowed_for(&[role("critic")])
            .contains(&"plugin_word_count".to_owned()),
        "Krytyczka bez grupy `plugin`"
    );
    plugins.disable("licznik").await.unwrap();
    assert_eq!(h.tools.names().len(), before, "wyłączona — znika od razu");
    let unavailable = harness(Vec::new(), false).tools.plugins();
    assert!(!unavailable.list().unwrap().available);
}

/// Komponent zwracający zawsze `{"words":2}` (import hosta, eksport `invoke`).
const PLUGIN: &str = r#"
(component
  (import "alfa:plugin/host@0.1.0" (instance $host
    (export "call" (func (param "op" string) (param "args" string) (result (result string (error string)))))
  ))
  (core module $Mem
    (memory (export "memory") 2)
    (global $bump (mut i32) (i32.const 4096))
    (func (export "realloc") (param i32 i32) (param $align i32) (param $size i32) (result i32)
      (local $p i32)
      (local.set $p (i32.and (i32.add (global.get $bump) (i32.sub (local.get $align) (i32.const 1)))
                             (i32.sub (i32.const 0) (local.get $align))))
      (global.set $bump (i32.add (local.get $p) (local.get $size)))
      (local.get $p))
  )
  (core instance $mem (instantiate $Mem))
  (alias core export $mem "memory" (core memory $memory))
  (alias core export $mem "realloc" (core func $realloc))
  (alias export $host "call" (func $host_call))
  (core func $call_lowered (canon lower (func $host_call) (memory $memory) (realloc $realloc)))
  (core module $Main
    (import "env" "memory" (memory 1))
    (import "env" "realloc" (func $realloc (param i32 i32 i32 i32) (result i32)))
    (import "host" "call" (func $call (param i32 i32 i32 i32 i32)))
    (data (i32.const 256) "{\"words\":2}")
    (func (export "invoke") (param i32 i32 i32 i32) (result i32)
      (local $r i32)
      (local.set $r (call $realloc (i32.const 0) (i32.const 0) (i32.const 4) (i32.const 12)))
      (i32.store8 (local.get $r) (i32.const 0))
      (i32.store offset=4 (local.get $r) (i32.const 256))
      (i32.store offset=8 (local.get $r) (i32.const 11))
      (local.get $r))
  )
  (core instance $main (instantiate $Main
    (with "env" (instance (export "memory" (memory $memory)) (export "realloc" (func $realloc))))
    (with "host" (instance (export "call" (func $call_lowered))))
  ))
  (func $invoke (param "tool" string) (param "input" string) (result (result string (error string)))
    (canon lift (core func $main "invoke") (memory $memory) (realloc $realloc)))
  (export "invoke" (func $invoke))
)
"#;
