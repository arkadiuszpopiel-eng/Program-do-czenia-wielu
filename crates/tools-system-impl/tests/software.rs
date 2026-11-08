//! `system_software`: lista zainstalowanych programów przez Brokera (odczyt informacji o systemie,
//! wynik niezaufany), filtr rodzaju, wynik trzymany w pamięci do `refresh`, brak wykrywania →
//! „niedostępne” zamiast pustej listy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{ctx, harness, sys};
use safety_broker_fake::ScriptedDecision;
use serde_json::json;
use tools_common_contract::{ToolErrorKind, ToolStatus};
use tools_system_contract::{SoftwareItem, SoftwareKind, SoftwareProbe, SystemToolsConfig};
use tools_system_impl::{SystemTools, SystemToolsDeps};

/// Atrapa wykrywania: stała lista i licznik pełnych wykryć.
#[derive(Default)]
struct Probe {
    scans: AtomicUsize,
}

fn item(id: &str, name: &str, kind: SoftwareKind, version: Option<&str>) -> SoftwareItem {
    SoftwareItem {
        id: id.into(),
        name: name.into(),
        kind,
        version: version.map(Into::into),
        command: id.into(),
    }
}

impl SoftwareProbe for Probe {
    fn scan(&self) -> Vec<SoftwareItem> {
        self.scans.fetch_add(1, Ordering::SeqCst);
        vec![
            item("node", "Node.js", SoftwareKind::Runtime, Some("20.11.1")),
            item("git", "Git", SoftwareKind::Vcs, Some("2.45.1")),
            item("edge", "Microsoft Edge", SoftwareKind::Browser, None),
        ]
    }
}

fn tools_with(probe: Option<Arc<Probe>>) -> (SystemTools, Arc<safety_broker_fake::FakeBroker>) {
    let h = harness(false);
    h.broker
        .script("tools-system.software", ScriptedDecision::Allow);
    let tools = SystemTools::new(SystemToolsDeps {
        sys: sys(),
        power: None,
        desktop: None,
        hardware: None,
        broker: h.broker.clone(),
        config: SystemToolsConfig::default(),
        bus: None,
        software: probe.map(|p| p as Arc<dyn SoftwareProbe>),
    });
    (tools, h.broker)
}

fn tool(tools: &SystemTools) -> Arc<dyn tools_common_contract::Tool> {
    use tools_common_contract::Toolset;
    tools
        .tools()
        .into_iter()
        .find(|t| t.manifest().name == "system_software")
        .unwrap()
}

#[tokio::test]
async fn lists_programs_by_kind_with_missing_common_tools_and_caches_the_scan() {
    let probe = Arc::new(Probe::default());
    let (tools, _broker) = tools_with(Some(probe.clone()));
    let t = tool(&tools);
    let out = t.call(json!({}), &ctx()).await;
    assert!(out.is_ok(), "{}", out.text);
    assert!(out.untrusted.is_some(), "lista programów to dane z systemu");
    assert!(out.text.contains("Node.js 20.11.1"), "{}", out.text);
    assert!(
        out.text.contains("Przeglądarki: Microsoft Edge"),
        "{}",
        out.text
    );
    // Popularne narzędzia, których nie ma, są nazwane — agentka ich nie zakłada.
    assert!(out.text.contains("Brak:"), "{}", out.text);
    assert!(
        out.data["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "python")
    );
    assert_eq!(out.data["items"].as_array().unwrap().len(), 3);

    let browsers = t.call(json!({ "kind": "browser" }), &ctx()).await;
    let items = browsers.data["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "edge");
    assert!(!browsers.text.contains("Brak:"), "{}", browsers.text);
    assert_eq!(
        probe.scans.load(Ordering::SeqCst),
        1,
        "drugi odczyt z pamięci"
    );

    let fresh = t.call(json!({ "refresh": true }), &ctx()).await;
    assert!(fresh.is_ok(), "{}", fresh.text);
    assert_eq!(
        probe.scans.load(Ordering::SeqCst),
        2,
        "`refresh` wykrywa na nowo"
    );
}

#[tokio::test]
async fn without_a_probe_the_tool_reports_unsupported_instead_of_an_empty_list() {
    let (tools, _broker) = tools_with(None);
    let out = tool(&tools).call(json!({}), &ctx()).await;
    assert_eq!(
        out.status,
        ToolStatus::Failed {
            error: ToolErrorKind::Unsupported
        },
        "{}",
        out.text
    );
    assert!(tools.software_summary().await.is_none());
}

#[tokio::test]
async fn unknown_arguments_are_rejected() {
    let (tools, _broker) = tools_with(Some(Arc::new(Probe::default())));
    let out = tool(&tools).call(json!({ "kind": "games" }), &ctx()).await;
    assert!(!out.is_ok(), "{}", out.text);
}
