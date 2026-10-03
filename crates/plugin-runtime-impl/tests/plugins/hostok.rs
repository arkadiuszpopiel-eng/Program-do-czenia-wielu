//! Operacje hosta — ścieżki dozwolone: odczyt w zadeklarowanym zakresie z tokenem Brokera
//! (unieważnionym po operacji, sesja oznaczona jako niezaufana), zapis po zgodzie
//! właściciela, `log` tylko do statystyk (zredagowany).

use std::time::Duration;

use plugin_runtime_contract::PluginLimits;
use safety_broker_contract::{ApprovalDecision, Broker};
use safety_broker_fake::ScriptedDecision;
use serde_json::json;

use crate::common::*;
use crate::hostcalls::{NOTE_A, NOTES, approver};

#[tokio::test(flavor = "multi_thread")]
async fn declared_read_goes_through_broker_token() {
    let h = harness();
    let wasm = component(&proxy_body("fs.read-text"));
    h.install(
        probe_manifest(
            "czytnik",
            &wasm,
            vec![fs_read_tree(NOTES)],
            PluginLimits::default(),
        ),
        wasm,
    )
    .await;
    let out = h
        .tool("plugin_probe")
        .call(json!({"path": NOTE_A}), &ctx())
        .await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(out.data, json!({"content": "notatka A"}));
    assert_eq!(
        out.untrusted,
        Some(plugin_runtime_contract::UNTRUSTED_SOURCE)
    );
    let calls = h.host.calls();
    assert_eq!(calls.len(), 1);
    assert!(
        calls[0].token
            && calls[0]
                .capability
                .as_deref()
                .is_some_and(|c| c.contains("a.txt"))
    );
    let sec = h
        .broker
        .session_security(&core_bus_contract::SessionId::new("s1"));
    assert!(sec.tainted, "wynik wtyczki oznacza sesję jako tainted");
    assert_eq!(
        h.broker.metrics().active_tokens,
        0,
        "token unieważniony po operacji"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn owner_approval_allows_declared_write() {
    let h = harness();
    h.broker
        .script("plugin.pisarz.probe", ScriptedDecision::NeedsApproval);
    tokio::spawn(approver(h.broker.clone(), ApprovalDecision::Allow));
    let mut c = ctx();
    c.approval_timeout = Duration::from_secs(5);
    let wasm = component(&proxy_body("fs.write-text"));
    let caps = vec![fs_write_tree(NOTES)];
    h.install(
        probe_manifest("pisarz", &wasm, caps, PluginLimits::default()),
        wasm,
    )
    .await;
    let path = r"C:\Users\user\Documents\notes\b.txt";
    let out = h
        .tool("plugin_probe")
        .call(json!({"path": path, "content": "nowa"}), &c)
        .await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(h.host.file(path).as_deref(), Some("nowa"));
}

#[tokio::test(flavor = "multi_thread")]
async fn log_lines_land_in_stats_not_in_world() {
    let h = harness();
    let wasm = component(
        r#"(data (i32.const 256) "log") (data (i32.const 600) "{}")
    (func (export "invoke") (param i32 i32) (param $ip i32) (param $il i32) (result i32)
      (local $r i32)
      (local.set $r (call $realloc (i32.const 0) (i32.const 0) (i32.const 4) (i32.const 12)))
      (call $call (i32.const 256) (i32.const 3) (local.get $ip) (local.get $il) (local.get $r))
      (call $ret (i32.const 0) (i32.const 600) (i32.const 2)))"#,
    );
    let out = h
        .probe(
            "dziennik",
            wasm,
            Vec::new(),
            PluginLimits::default(),
            json!({"message": "start\u{0007} sk-ant-api03-BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"}),
        )
        .await;
    assert!(out.is_ok(), "{}", out.text);
    let invoked = h
        .bus
        .recorded()
        .into_iter()
        .find(|e| e.kind.as_str() == "plugin.invoked")
        .unwrap();
    let logs = invoked.payload["logs"].to_string();
    assert!(
        logs.contains("start") && !logs.contains("BBBBBBBBBBBBBBBB"),
        "{logs}"
    );
    assert_eq!(invoked.payload["host_calls"], 1);
}
