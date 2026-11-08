//! Narzędzia przeglądarki C6.4 na atrapie: przewijanie i konsola bez Brokera (odczyt, treść
//! niezaufana, sekrety redagowane), klawisz / wybór / historia ze zgodą `net.egress(host)`
//! i odmową bez niej, czekanie na tekst z limitem i anulowaniem.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use compliance_contract::{DenyLists, PathEnv};
use platform_apps_contract::{BrowserKind, BrowserSpec, ConsoleEntry, ConsoleLevel};
use platform_apps_fake::{FakeBrowser, FakeNode, FakePage};
use safety_broker_contract::{Holder, KernelPolicy, TaintSource};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_browser_contract::BrowserToolsConfig;
use tools_browser_impl::{BrowserTools, BrowserToolsDeps};
use tools_common_contract::{Tool, ToolCtx, ToolOutcome, ToolStatus, Toolset};
use watchdog_contract::ManualClock;

const HOME: &str = "/Users/ala";

/// Przykładowy token w treści strony (składany w czasie działania — bez sekretu w źródle).
fn fake_token() -> String {
    format!("{}_{}", "ghp", "abcdefghijklmnopqrstuvwxyz0123")
}

fn abs(rel: &str) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" });
    rel.split('/')
        .filter(|s| !s.is_empty())
        .fold(root, |acc, seg| acc.join(seg))
}

struct H {
    web: Arc<FakeBrowser>,
    broker: Arc<FakeBroker>,
    tools: BrowserTools,
}

fn harness() -> H {
    let env = PathEnv::windows_profile(HOME);
    let policy = KernelPolicy::baseline(HOME, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    for t in ["open", "type", "key", "select", "nav"] {
        broker.script(&format!("tools-browser.{t}"), ScriptedDecision::Allow);
    }
    let web = Arc::new(FakeBrowser::new());
    let mut error = ConsoleEntry::new(
        ConsoleLevel::Error,
        "exception",
        &format!("TypeError: x is undefined; token {}", fake_token()),
    );
    error.url = Some("http://localhost:5173/app.js".into());
    error.line = Some(12);
    web.add_page(
        "http://localhost:5173/",
        FakePage {
            title: "Moja aplikacja".into(),
            text: "Formularz zamówienia. Gotowe.".into(),
            nodes: vec![
                FakeNode {
                    options: vec!["Polska".into(), "Niemcy".into()],
                    ..FakeNode::new("combobox", "Kraj")
                },
                FakeNode {
                    href: Some("http://localhost:5173/wynik".into()),
                    ..FakeNode::new("textbox", "Szukaj")
                },
            ],
            console: vec![
                ConsoleEntry::new(ConsoleLevel::Log, "console", "start"),
                error,
            ],
            ..FakePage::default()
        },
    );
    web.add_page(
        "http://localhost:5173/wynik",
        FakePage {
            title: "Wynik".into(),
            ..FakePage::default()
        },
    );
    let tools = BrowserTools::new(BrowserToolsDeps {
        browser: web.clone(),
        broker: broker.clone(),
        spec: BrowserSpec {
            kind: BrowserKind::Edge,
            executable: None,
            alfa_root: abs("Users/ala/AppData/Local/Alfa"),
            profile_dir: abs("Users/ala/AppData/Local/Alfa/browser/profile"),
            quarantine_dir: abs("Users/ala/AppData/Local/Alfa/browser/quarantine"),
            headless: true,
        },
        deny: DenyLists::baseline(),
        env,
        config: BrowserToolsConfig::default(),
        bus: None,
    });
    H { web, broker, tools }
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_millis(200);
    c
}

impl H {
    fn tool(&self, n: &str) -> Arc<dyn Tool> {
        self.tools
            .tools()
            .into_iter()
            .find(|t| t.manifest().name == n)
            .unwrap()
    }

    async fn call(&self, n: &str, args: serde_json::Value) -> ToolOutcome {
        self.tool(n).call(args, &ctx()).await
    }
}

#[tokio::test]
async fn page_operations_for_testing_own_pages() {
    let h = harness();
    let none = h.call("browser_console", json!({})).await;
    assert!(none.text.contains("browser_open"), "{}", none.text);
    let open = h
        .call("browser_open", json!({"url": "http://localhost:5173/"}))
        .await;
    assert!(open.is_ok(), "{}", open.text);

    let console = h
        .call("browser_console", json!({"errors_only": true}))
        .await;
    assert!(console.is_ok());
    assert_eq!(console.untrusted, Some(TaintSource::Web));
    assert_eq!(console.data["errors"], 1);
    assert!(console.text.contains("TypeError") && console.text.contains("app.js:12"));
    assert!(!console.text.contains(&fake_token()), "sekret zredagowany");
    let again = h.call("browser_console", json!({})).await;
    assert!(again.text.contains("pusta"), "odczyt opróżnia konsolę");

    assert!(
        h.call("browser_scroll", json!({"pixels": 800}))
            .await
            .is_ok()
    );
    assert!(h.call("browser_scroll", json!({"node": 1})).await.is_ok());
    assert!(
        !h.call("browser_scroll", json!({"pixels": 99_999}))
            .await
            .is_ok()
    );

    let bad = h
        .call("browser_select", json!({"node": 1, "value": "Francja"}))
        .await;
    assert!(bad.text.contains("Polska, Niemcy"), "{}", bad.text);
    assert!(
        h.call("browser_select", json!({"node": 1, "value": "niemcy"}))
            .await
            .is_ok()
    );
    assert!(
        h.call("browser_type", json!({"node": 2, "text": "buty"}))
            .await
            .is_ok()
    );
    let enter = h.call("browser_key", json!({"key": "enter"})).await;
    assert!(enter.is_ok(), "{}", enter.text);
    assert!(
        enter.text.contains("wynik"),
        "Enter wysłał formularz: {}",
        enter.text
    );

    let back = h.call("browser_nav", json!({"action": "back"})).await;
    assert!(back.is_ok(), "{}", back.text);
    let found = h
        .call("browser_wait", json!({"text": "gotowe", "timeout_ms": 500}))
        .await;
    assert_eq!(found.data["found"], true);
    assert!(
        h.call("browser_nav", json!({"action": "reload"}))
            .await
            .is_ok()
    );
    let missing = h
        .call(
            "browser_wait",
            json!({"text": "nie ma takiego", "timeout_ms": 400}),
        )
        .await;
    assert_eq!(missing.data["found"], false);
    assert!(missing.data["waited_ms"].as_u64().unwrap() >= 300);

    let log = h.web.actions().0;
    assert!(log.contains(&"scroll:+800".to_owned()));
    assert!(log.contains(&"select:1=niemcy".to_owned()));
    assert!(log.iter().any(|a| a == "key:Enter"));
    assert!(log.iter().any(|a| a == "history:Back"));
}

#[tokio::test]
async fn actions_need_broker_and_wait_is_cancellable() {
    let h = harness();
    assert!(
        h.call("browser_open", json!({"url": "http://localhost:5173/"}))
            .await
            .is_ok()
    );
    h.broker.script(
        "tools-browser.key",
        ScriptedDecision::Deny(risk_classifier_contract::KernelRule::ProviderWebUi),
    );
    let denied = h.call("browser_key", json!({"key": "enter"})).await;
    assert!(
        matches!(denied.status, ToolStatus::Denied { .. }),
        "{denied:?}"
    );
    assert!(
        !h.web.actions().0.iter().any(|a| a.starts_with("key:")),
        "bez zgody — bez klawisza"
    );

    let c = ctx();
    let cancel = c.cancel.clone();
    let wait = h.tool("browser_wait");
    let task = tokio::spawn(async move {
        wait.call(json!({"text": "nigdy", "timeout_ms": 20_000}), &c)
            .await
    });
    tokio::time::sleep(Duration::from_millis(350)).await;
    cancel.cancel();
    let out = task.await.unwrap();
    assert!(matches!(out.status, ToolStatus::Cancelled));
}
