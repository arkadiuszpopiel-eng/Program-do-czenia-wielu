//! Wyszukanie bezAPI to nawigacja przez ten sam Broker i BrowserPort, nie ranking wyników.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::{DenyLists, PathEnv};
use platform_apps_contract::{BrowserKind, BrowserSpec};
use platform_apps_fake::{FakeBrowser, FakePage};
use risk_classifier_contract::KernelRule;
use safety_broker_contract::{Holder, KernelPolicy, TaintSource};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tools_browser_contract::BrowserToolsConfig;
use tools_browser_impl::{BrowserTools, BrowserToolsDeps};
use tools_common_contract::{Tool, ToolCtx, ToolStatus, Toolset};
use watchdog_contract::ManualClock;

const URL: &str = "https://duckduckgo.com/?q=Rust+async";
fn abs(rel: &str) -> PathBuf {
    rel.split('/').filter(|s| !s.is_empty()).fold(
        PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" }),
        |p, s| p.join(s),
    )
}
fn harness(allowed: bool) -> (Arc<FakeBrowser>, Arc<FakeBroker>, BrowserTools, ToolCtx) {
    let env = PathEnv::windows_profile("/Users/ala");
    let broker = Arc::new(
        FakeBroker::with(
            KernelPolicy::baseline("/Users/ala", "/ProgramData/AlfaBroker").unwrap(),
            env.clone(),
            Arc::new(ManualClock::new(1_000_000)),
        )
        .unwrap(),
    );
    if allowed {
        broker.script("tools-browser.search", ScriptedDecision::Allow);
    }
    let web = Arc::new(FakeBrowser::new());
    web.add_page(
        URL,
        FakePage {
            title: "Wyniki".into(),
            text: "Nieprzeczytana jeszcze strona".into(),
            resources: vec!["https://tracker.example/pixel".into()],
            ..FakePage::default()
        },
    );
    let tools = BrowserTools::new(BrowserToolsDeps {
        browser: web.clone(),
        broker: broker.clone(),
        spec: BrowserSpec {
            kind: BrowserKind::Edge,
            executable: None,
            alfa_root: abs("Users/ala/Alfa"),
            profile_dir: abs("Users/ala/Alfa/browser/profile"),
            quarantine_dir: abs("Users/ala/Alfa/browser/quarantine"),
            headless: true,
        },
        deny: DenyLists::baseline(),
        env,
        config: BrowserToolsConfig::default(),
        bus: None,
    });
    let mut ctx = ToolCtx::new(Holder::agent("synthetic-search", "delta"));
    ctx.approval_timeout = Duration::from_millis(50);
    (web, broker, tools, ctx)
}
fn search(tools: &BrowserTools) -> Arc<dyn Tool> {
    tools
        .tools()
        .into_iter()
        .find(|t| t.manifest().name == "browser_search")
        .expect("Brakuje browser_search w rzeczywistym zestawie narzędzi")
}
#[tokio::test]
async fn search_opens_engine_page_under_broker_but_does_not_claim_results_read() {
    let (web, broker, tools, ctx) = harness(true);
    let result = search(&tools)
        .call(json!({"engine":"duckduckgo","query":"Rust async"}), &ctx)
        .await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(result.untrusted, Some(TaintSource::Web));
    assert_eq!(result.data["url"], URL);
    assert_eq!(result.data["search_engine"], "duckduckgo");
    assert_eq!(result.data["search_stage"], "navigation_only");
    assert!(!result.text.contains("Nieprzeczytana jeszcze strona"));
    assert_eq!(result.data["blocked_hosts"], json!(["tracker.example"]));
    assert_eq!(
        web.network().iter().filter(|(_, allowed)| *allowed).count(),
        1
    );
    assert!(
        broker
            .audit_names()
            .iter()
            .any(|s| s == "broker.token.issued")
    );
    assert_eq!(web.open_sessions(), 1);
    let typed: tools_browser_contract::SearchOutput = serde_json::from_value(result.data).unwrap();
    assert_eq!(
        typed.search_stage,
        tools_browser_contract::SearchStage::NavigationOnly
    );
    let read = tools
        .tools()
        .into_iter()
        .find(|t| t.manifest().name == "browser_read")
        .unwrap()
        .call(json!({}), &ctx)
        .await;
    assert!(read.is_ok() && read.text.contains("Nieprzeczytana jeszcze strona"));
}

#[tokio::test]
async fn denial_has_zero_launches_and_zero_network() {
    let (web, _broker, tools, ctx) = harness(false);
    let result = search(&tools)
        .call(json!({"engine":"duckduckgo","query":"Rust async"}), &ctx)
        .await;
    assert!(!result.is_ok(), "{result:?}");
    assert!(web.launches().is_empty());
    assert!(web.network().is_empty());
    assert_eq!(web.open_sessions(), 0);
}

#[tokio::test]
async fn cancel_before_call_has_zero_launches_and_zero_network() {
    let (web, _broker, tools, ctx) = harness(true);
    ctx.cancel.cancel();
    let result = search(&tools)
        .call(json!({"engine":"duckduckgo","query":"Rust async"}), &ctx)
        .await;
    assert_eq!(result.status, ToolStatus::Cancelled);
    assert!(web.launches().is_empty() && web.network().is_empty());
}

#[tokio::test]
async fn invalid_search_never_asks_broker_or_reaches_browser() {
    let (web, broker, tools, ctx) = harness(true);
    for args in [
        json!({"query":"Rust"}),
        json!({"engine":"duckduckgo","query":"Rust","extra_hosts":["evil.example"]}),
        json!({"engine":"duckduckgo","query":""}),
        json!({"engine":"https://evil.example","query":"Rust"}),
    ] {
        let result = search(&tools).call(args, &ctx).await;
        assert!(!result.is_ok(), "{result:?}");
    }
    assert!(web.launches().is_empty() && web.network().is_empty());
    assert!(
        !broker
            .audit_names()
            .iter()
            .any(|s| s == "broker.token.issued")
    );
}

#[tokio::test]
async fn every_search_rechecks_broker_even_for_an_already_approved_host() {
    let (web, broker, tools, ctx) = harness(true);
    let tool = search(&tools);
    let args = json!({"engine":"duckduckgo","query":"Rust async"});
    assert!(tool.call(args.clone(), &ctx).await.is_ok());
    let before = web.network();
    broker.script(
        "tools-browser.search",
        ScriptedDecision::Deny(KernelRule::ProviderWebUi),
    );
    assert!(!tool.call(args, &ctx).await.is_ok());
    assert_eq!(web.network(), before);
    assert_eq!(web.launches().len(), 1);
    assert_eq!(tools.close_all(), 1);
    assert_eq!(web.open_sessions(), 0);
}
