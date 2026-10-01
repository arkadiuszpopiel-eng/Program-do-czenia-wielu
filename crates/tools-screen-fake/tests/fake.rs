//! Atrapa przechodzi testy kontraktowe i oznacza odczyty jako niezaufane.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use tools_common_contract::{ToolCtx, ToolOutcome, Toolset};
use tools_screen_fake::FakeTools;

#[tokio::test]
async fn contract_and_scripting() {
    let fake = FakeTools::default();
    tools_screen_contract::contract_tests::run_all(&fake.tools()).await;
    let first = fake.tools().remove(0);
    let name = first.manifest().name.clone();
    fake.push(
        &name,
        ToolOutcome::ok("skrypt", serde_json::json!({"x": 1})),
    );
    let ctx = ToolCtx::new(safety_broker_contract::Holder::agent("s1", "delta"));
    let out = first
        .call(tools_screen_contract::sample_args(&name), &ctx)
        .await;
    assert_eq!(out.text, "skrypt");
    assert_eq!(out.untrusted, first.manifest().untrusted_output.clone());
    assert!(fake.calls().iter().any(|(t, _)| *t == name));
}
