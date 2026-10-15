//! Narzędzia agentek `mcp_tools` / `mcp_call`: kontrakt, tylko narzędzia ze zgodą, Broker
//! (zgoda, odmowa, zatwierdzenie), wynik niezaufany, anulowanie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{input, setup};
use mcp_fake::{FakeMcpServer, simple_tool};
use risk_classifier_contract::KernelRule;
use safety_broker_contract::{ApprovalDecision, Holder, TaintSource};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{BrokerGate, Tool, ToolCtx, ToolStatus};

struct Env {
    s: common::Setup,
    broker: Arc<FakeBroker>,
    list: Arc<dyn Tool>,
    call: Arc<dyn Tool>,
    fake: FakeMcpServer,
}

async fn env(tag: &str) -> Env {
    let s = setup(tag);
    s.hub.save(input("Notatki")).await.unwrap();
    let fake = FakeMcpServer::new(vec![
        simple_tool("dodaj", "Dodaje notatkę."),
        simple_tool("usun", "Usuwa notatkę."),
    ]);
    s.inproc
        .servers
        .lock()
        .unwrap()
        .insert("notatki".into(), fake.clone());
    let t = s.hub.tools_ipc("notatki").await.unwrap();
    let dodaj = t.tools.iter().find(|x| x.name == "dodaj").unwrap();
    s.hub
        .approve_ipc("notatki", "dodaj", &dodaj.fingerprint)
        .await
        .unwrap();
    let broker = Arc::new(FakeBroker::new().unwrap());
    broker.script("app-mcp.tools", ScriptedDecision::Allow);
    broker.script("app-mcp.call", ScriptedDecision::Allow);
    let tools = app_mcp::agent_tools(
        s.hub.clone(),
        BrokerGate::new(broker.clone()).with_poll(Duration::from_millis(5)),
    );
    Env {
        s,
        broker,
        list: tools[0].clone(),
        call: tools[1].clone(),
        fake,
    }
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_millis(500);
    c
}

#[tokio::test]
async fn manifests_follow_contract() {
    let e = env("contract").await;
    for t in [&e.list, &e.call] {
        tools_common_contract::contract_tests::manifest_is_valid(t.as_ref());
        tools_common_contract::contract_tests::invalid_args_rejected(t.as_ref(), "C:\\").await;
    }
    assert_eq!(e.list.manifest().name, "mcp_tools");
    assert!(!e.list.manifest().mutating && e.call.manifest().mutating);
    assert_eq!(e.call.manifest().groups, vec!["mcp".to_owned()]);
}

#[tokio::test]
async fn list_shows_only_approved_tools() {
    let e = env("list").await;
    let out = e.list.call(json!({}), &ctx()).await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(out.untrusted, Some(TaintSource::Mcp));
    let servers = out.data["servers"].as_array().unwrap();
    assert_eq!(servers.len(), 1);
    let names: Vec<&str> = servers[0]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["dodaj"]);
    assert_eq!(servers[0]["awaiting_consent"], 1);
    assert!(out.text.contains("1 czeka na zgodę"), "{}", out.text);
    // Filtr serwera nieznanego — pusta lista bez błędu.
    let none = e.list.call(json!({"server": "inny"}), &ctx()).await;
    assert!(none.is_ok() && none.text.contains("Brak włączonych"));
}

#[tokio::test]
async fn call_runs_approved_tool_and_marks_untrusted() {
    let e = env("call").await;
    let out = e
        .call
        .call(
            json!({"server": "notatki", "tool": "dodaj", "arguments": {"tekst": "mleko"}}),
            &ctx(),
        )
        .await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(out.untrusted, Some(TaintSource::Mcp));
    assert_eq!(out.data["echo"]["tekst"], "mleko");
    assert_eq!(e.fake.calls().len(), 1);
    // Bez zgody — błąd czytelny, serwer nie dostaje wywołania.
    let blocked = e
        .call
        .call(json!({"server": "notatki", "tool": "usun"}), &ctx())
        .await;
    assert!(matches!(blocked.status, ToolStatus::Failed { .. }));
    assert!(blocked.text.contains("zgod"), "{}", blocked.text);
    assert_eq!(e.fake.calls().len(), 1);
    // Nieznany serwer, argumenty nie-obiekt.
    let unknown = e
        .call
        .call(json!({"server": "nie-ma", "tool": "x"}), &ctx())
        .await;
    assert!(unknown.text.contains("Nie ma serwera"));
    let bad = e
        .call
        .call(
            json!({"server": "notatki", "tool": "dodaj", "arguments": [1]}),
            &ctx(),
        )
        .await;
    assert!(matches!(bad.status, ToolStatus::Failed { .. }));
    assert_eq!(e.fake.calls().len(), 1);
    drop(e.s);
}

#[tokio::test]
async fn broker_denial_stops_call() {
    let e = env("deny").await;
    e.broker.script(
        "app-mcp.call",
        ScriptedDecision::Deny(KernelRule::GuiControlOfKernelProcess),
    );
    let out = e
        .call
        .call(json!({"server": "notatki", "tool": "dodaj"}), &ctx())
        .await;
    assert!(matches!(out.status, ToolStatus::Denied { .. }), "{out:?}");
    assert!(e.fake.calls().is_empty());
}

#[tokio::test]
async fn broker_approval_card_then_call() {
    let e = env("approve").await;
    e.broker
        .script("app-mcp.call", ScriptedDecision::NeedsApproval);
    let call = e.call.clone();
    let task = tokio::spawn(async move {
        call.call(json!({"server": "notatki", "tool": "dodaj"}), &ctx())
            .await
    });
    let mut approved = 0;
    for _ in 0..100 {
        approved = e.broker.auto_approve(ApprovalDecision::Allow).await;
        if approved > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(approved, 1);
    let out = task.await.unwrap();
    assert!(out.is_ok(), "{}", out.text);
    assert!(out.approval.is_some());
    assert_eq!(e.fake.calls().len(), 1);
}

#[tokio::test]
async fn cancelled_context_does_not_call() {
    let e = env("cancel").await;
    let c = ctx();
    c.cancel.cancel();
    let out = e
        .call
        .call(json!({"server": "notatki", "tool": "dodaj"}), &c)
        .await;
    assert!(matches!(out.status, ToolStatus::Cancelled));
    assert!(e.fake.calls().is_empty());
}
