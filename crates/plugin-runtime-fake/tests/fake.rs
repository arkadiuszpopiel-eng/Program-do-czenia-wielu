//! Atrapa przechodzi ten sam test kontraktowy co `-impl`; zachowania i obróbka wyniku.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use plugin_runtime_contract::{
    COMPONENT_HEADER, ExecError, PluginApproval, PluginSource, Plugins, UNTRUSTED_SOURCE,
    contract_tests, samples, sha256_hex,
};
use plugin_runtime_fake::{FakePlugins, word_count_behavior};
use safety_broker_contract::Holder;
use serde_json::json;
use tools_common_contract::{ToolCtx, ToolStatus};

fn wasm(n: u8) -> Vec<u8> {
    let mut b = COMPONENT_HEADER.to_vec();
    b.push(n);
    b
}

fn ctx() -> ToolCtx {
    ToolCtx::new(Holder::agent("s1", "delta"))
}

async fn installed(fake: &FakePlugins, n: u8) {
    let r = fake
        .propose(
            samples::manifest("licznik", "1.0.0", &wasm(n)),
            wasm(n),
            PluginSource::User,
        )
        .await
        .unwrap();
    fake.approve(
        &r.manifest.id,
        &r.manifest.version,
        PluginApproval::ui(&r.review_hash),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn contract_lifecycle_on_fake() {
    let fake = FakePlugins::new();
    contract_tests::lifecycle(&fake, &wasm).await;
    assert!(
        fake.events()
            .iter()
            .any(|e| e.kind.as_str() == "plugin.installed")
    );
}

#[tokio::test]
async fn behaviors_and_output_checks() {
    let fake = FakePlugins::default();
    installed(&fake, 1).await;
    let tool = fake.tools().pop().unwrap();
    let out = tool.call(json!({"text": "Ala ma kota"}), &ctx()).await;
    assert_eq!(out.data, json!({"words": 3}));
    assert_eq!(out.untrusted, Some(UNTRUSTED_SOURCE));
    assert_eq!(fake.calls().len(), 1);

    let sha = sha256_hex(&wasm(1));
    fake.behave(&sha, Arc::new(|_, _| Ok(json!([1, 2]))));
    let out = tool.call(json!({"text": "x"}), &ctx()).await;
    assert_eq!(out.data["error"], "invalid_output");
    fake.behave(
        &sha,
        Arc::new(|_, _| Err(ExecError::PluginFailed("zła wtyczka".into()))),
    );
    let out = tool.call(json!({"text": "x"}), &ctx()).await;
    assert_eq!(out.data["error"], "plugin_failed");
    assert_eq!(out.untrusted, Some(UNTRUSTED_SOURCE));
    fake.behave(&sha, Arc::new(|_, _| Err(ExecError::OutOfFuel)));
    let out = tool.call(json!({"text": "x"}), &ctx()).await;
    assert!(matches!(out.status, ToolStatus::Failed { .. }));
    assert!(
        fake.events()
            .iter()
            .any(|e| e.kind.as_str() == "plugin.trapped")
    );

    let big = "x".repeat(70_000);
    let out = tool.call(json!({"text": big}), &ctx()).await;
    assert_eq!(out.data["error"], "input_too_large");

    let c = ctx();
    c.cancel.cancel();
    assert_eq!(
        tool.call(json!({"text": "x"}), &c).await.status,
        ToolStatus::Cancelled
    );

    fake.disable(&samples::manifest("licznik", "1.0.0", &wasm(1)).id)
        .await
        .unwrap();
    assert_eq!(
        tool.call(json!({"text": "x"}), &ctx()).await.data["error"],
        "unavailable"
    );
}

#[tokio::test]
async fn module_without_behavior_traps_and_clock_advances() {
    let fake = FakePlugins::new();
    fake.default_behavior(None);
    fake.advance(5_000);
    installed(&fake, 2).await;
    assert_eq!(fake.list()[0].proposed_at_ms, 5_000);
    let out = fake.tools()[0].call(json!({"text": "x"}), &ctx()).await;
    assert_eq!(out.data["error"], "trap");
    fake.default_behavior(Some(word_count_behavior()));
    let out = fake.tools()[0]
        .call(json!({"text": "raz, dwa"}), &ctx())
        .await;
    assert_eq!(out.data, json!({"words": 2}));
}
