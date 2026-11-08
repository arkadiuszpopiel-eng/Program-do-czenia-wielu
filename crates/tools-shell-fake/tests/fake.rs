//! Atrapa przechodzi testy kontraktowe i odtwarza skrypt.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use safety_broker_contract::Holder;
use tools_common_contract::{ToolCtx, ToolOutcome, Toolset};
use tools_shell_fake::FakeShellTools;

#[tokio::test]
async fn contract_suite_passes_on_fake() {
    let fake = FakeShellTools::new();
    tools_shell_contract::contract_tests::run_all(&fake.tools(), "/w").await;
}

#[tokio::test]
async fn scripted_run() {
    let fake = FakeShellTools::default();
    fake.push_run(ToolOutcome::ok(
        "kod 0",
        serde_json::json!({"exit_code": 0}),
    ));
    fake.set_default_run(ToolOutcome::ok("domyślnie", serde_json::json!({})));
    let ctx = ToolCtx::new(Holder::agent("s1", "delta"));
    let run = fake.tools().into_iter().next().unwrap();
    let a = run.call(serde_json::json!({"command": "dir"}), &ctx).await;
    let b = run.call(serde_json::json!({"command": "dir"}), &ctx).await;
    assert_eq!((a.text.as_str(), b.text.as_str()), ("kod 0", "domyślnie"));
    assert_eq!(fake.run_calls().len(), 2);
    assert!(fake.terminal_calls().is_empty());
}
