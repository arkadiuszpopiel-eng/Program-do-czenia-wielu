//! Atrapa przechodzi testy kontraktowe, oznacza odczyty jako niezaufane i odrzuca odmowy
//! polityki bez zapisu wywołania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use tools_common_contract::contract_tests::ctx;
use tools_common_contract::{DenialReason, ToolOutcome, ToolStatus, Toolset};
use tools_system_fake::FakeTools;

#[tokio::test]
async fn contract_and_scripting() {
    let fake = FakeTools::default();
    tools_system_contract::contract_tests::run_all(&fake.tools()).await;
    let calls_after_contract = fake.calls().len();
    let tools = fake.tools();
    let list = tools
        .iter()
        .find(|t| t.manifest().name == "system_processes")
        .unwrap();
    fake.push(
        "system_processes",
        ToolOutcome::ok("3 procesy", serde_json::json!({})),
    );
    let out = list.call(serde_json::json!({}), &ctx("/")).await;
    assert_eq!(out.text, "3 procesy");
    assert_eq!(out.untrusted, list.manifest().untrusted_output.clone());
    let set = tools
        .iter()
        .find(|t| t.manifest().name == "system_env_set")
        .unwrap();
    let bad = set
        .call(
            serde_json::json!({"name": "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "value": "--remote-debugging-port=9222"}),
            &ctx("/"),
        )
        .await;
    assert_eq!(
        bad.status,
        ToolStatus::Denied {
            reason: DenialReason::Policy
        }
    );
    assert_eq!(
        fake.calls().len(),
        calls_after_contract + 1,
        "odmowa polityki bez zapisu wywołania"
    );
}
