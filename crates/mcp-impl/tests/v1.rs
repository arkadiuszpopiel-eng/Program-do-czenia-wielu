//! Serwer MCP Alfy v1 (F6): widoczność narzędzi v1, narzędzia UIA i zrzutów przez narzędzia agentek
//! (podmiot „most CLI”, argumenty niezaufane, wynik `unverified_by_alfa`), zgodność schematów
//! z narzędziami agentek i odcisk definicji, host z kanałem lokalnym (bez TCP).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod v1_common;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mcp_contract::contract_tests::ProxySide;
use mcp_contract::{AlfaTool, BridgeMcpHost, BridgeScope, ToolCallError, ToolHandler};
use mcp_impl::{AlfaToolHandler, HostConfig, LocalMcpHost, McpV1, MonotonicClock, listener};
use safety_broker_contract::TaintSource;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use tools_common_contract::{DenialReason, ToolOutcome, Toolset};
use v1_common::{env, handler, ports};

fn names(h: &AlfaToolHandler) -> BTreeSet<String> {
    h.tools().into_iter().map(|t| t.name).collect()
}

#[test]
fn v1_tools_visible_only_with_scope_and_configuration() {
    let e = env(true);
    let v1 = names(&handler(&e, &BridgeScope::windows_v1("z")));
    for t in AlfaTool::V1_ONLY {
        assert!(v1.contains(t.name()), "{}", t.name());
    }
    assert!(!v1.contains("approve"));
    let v0 = names(&handler(&e, &BridgeScope::windows_v0("z")));
    assert!(
        v0.iter()
            .all(|n| AlfaTool::from_name(n).is_some_and(|t| !t.is_v1()))
    );
    let scope = BridgeScope::windows_v1("z");
    let without = AlfaToolHandler::new(&scope.tools, ports(), None).with_v1(
        &scope,
        None,
        CancellationToken::new(),
    );
    assert_eq!(
        names(&without).len(),
        4,
        "bez konfiguracji v1 tylko narzędzia v0"
    );
}

#[tokio::test]
async fn gui_tools_run_as_bridge_with_unverified_results() {
    let e = env(true);
    let h = handler(
        &e,
        &BridgeScope::windows_v1("zadanie-7").with_session("s-czat"),
    );
    e.uia.push(
        "uia_tree",
        ToolOutcome::ok("drzewo", json!({"window": 5, "nodes": []})).untrusted(TaintSource::Screen),
    );
    let out = h.call("uia_tree", json!({"window": 5})).await.unwrap();
    assert!(!out.is_error);
    assert_eq!(
        out.structured_content.as_ref().unwrap()["unverified_by_alfa"],
        true
    );
    let (_, call) = e.uia.calls().pop().unwrap();
    assert_eq!(call.holder.agent.unwrap().as_str(), "most-cli");
    assert_eq!(call.holder.session.as_str(), "s-czat");
    assert!(
        call.untrusted_args,
        "argumenty mostu traktowane jak niezaufane"
    );
    let denied = ToolOutcome::denied(DenialReason::Policy, "akcja");
    e.uia.push("uia_act", denied);
    let act = h
        .call("uia_act", json!({"element": "w1:1.2", "action": "invoke"}))
        .await;
    assert!(matches!(act, Err(ToolCallError::Unauthorized(_))));
    let bad = h
        .call(
            "uia_act",
            json!({"element": "w1:1.2", "action": "set_value"}),
        )
        .await;
    assert!(matches!(bad, Err(ToolCallError::InvalidParams(_))));
    let mut shot = ToolOutcome::ok("zrzut", json!({"width": 10}));
    shot.images.push(tools_common_contract::ToolImage {
        media_type: "image/png".into(),
        data_base64: "AAAA".into(),
    });
    e.screen.push("screen_capture", shot);
    let img = h
        .call("screen_capture", json!({"target": "monitor"}))
        .await
        .unwrap();
    assert!(
        img.content
            .iter()
            .any(|c| matches!(c, mcp_contract::Content::Image { .. }))
    );
    let other = handler(&e, &BridgeScope::windows_v1("bez-sesji"));
    other
        .call("uia_find", json!({"window": 1, "role": "button"}))
        .await
        .unwrap();
    assert_eq!(
        e.uia.calls().pop().unwrap().1.holder.session.as_str(),
        "most:bez-sesji"
    );
}

fn props(schema: &Value) -> (BTreeSet<String>, BTreeSet<String>) {
    let keys = schema["properties"]
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    let req = schema["required"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    (keys, req)
}

#[test]
fn mcp_schemas_match_agent_tool_arguments() {
    let mut manifests = tools_uia_contract::manifests();
    manifests.extend(tools_screen_contract::manifests());
    for t in AlfaTool::V1_ONLY
        .into_iter()
        .filter(|t| *t != AlfaTool::RegistryRead)
    {
        let m = manifests.iter().find(|m| m.name == t.name()).unwrap();
        assert_eq!(
            props(&t.definition().input_schema),
            props(&m.input_schema),
            "{}",
            t.name()
        );
    }
    assert_eq!(
        mcp_contract::alfa_tools_fingerprint(),
        mcp_contract::ALFA_TOOLS_FINGERPRINT
    );
}

#[tokio::test]
async fn host_serves_v1_over_local_channel_without_tcp() {
    let e = env(true);
    let config = HostConfig::new(PathBuf::from("alfa-mcp-proxy"));
    let v1 = McpV1 {
        gui_tools: e.uia.tools(),
        registry: e.registry.clone(),
        broker: e.broker.clone(),
        bus: None,
        approval_timeout: Duration::from_millis(200),
    };
    let host = LocalMcpHost::start_with(
        config,
        ports(),
        Some(v1),
        Arc::new(MonotonicClock::default()),
    )
    .unwrap();
    assert!(!host.endpoint().to_env_value().starts_with("tcp"));
    let reg = host
        .register(BridgeScope::windows_v1("host"), None)
        .await
        .unwrap();
    let token = reg.launch.token().unwrap().to_owned();
    let stream = listener::connect(host.endpoint()).await.unwrap();
    let mut side = ProxySide::hello(stream, &token).await;
    side.initialize().await.unwrap();
    let list = side.request("tools/list", json!({})).await.unwrap();
    let tools: BTreeSet<String> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(tools.contains("uia_tree") && tools.contains("registry_read"));
    assert!(
        !tools.contains("screen_capture"),
        "brak wstrzykniętego narzędzia zrzutów → niewidoczne"
    );
    let call = side
        .request("tools/call", json!({"name": "registry_read", "arguments": {"key": r"HKCU\Software\Acme\Ustawienia"}}))
        .await
        .unwrap();
    assert_eq!(
        call["result"]["structuredContent"]["unverified_by_alfa"],
        true
    );
    host.revoke(&reg.id).await.unwrap();
}
