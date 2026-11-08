//! Atrapa przechodzi testy kontraktowe i odtwarza skrypt.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use safety_broker_contract::{Holder, TaintSource};
use tools_common_contract::{ToolCtx, ToolOutcome, ToolStatus, Toolset};
use tools_fs_contract::FsToolKind;
use tools_fs_fake::FakeFsTools;

#[tokio::test]
async fn contract_suite_passes_on_fake() {
    let fake = FakeFsTools::new();
    tools_fs_contract::contract_tests::run_all(&fake.tools(), "/w").await;
}

#[tokio::test]
async fn scripted_results_and_calls() {
    let fake = FakeFsTools::default();
    fake.push(
        FsToolKind::Read,
        ToolOutcome::ok("treść", serde_json::json!({})).untrusted(TaintSource::File),
    );
    fake.set_default(FsToolKind::Write, ToolOutcome::cancelled("zapis"));
    let ctx = ToolCtx::new(Holder::agent("s1", "delta"));
    let read = fake.tool(FsToolKind::Read).unwrap();
    let out = read.call(serde_json::json!({"path": "/a"}), &ctx).await;
    assert_eq!(out.untrusted, Some(TaintSource::File));
    let again = read.call(serde_json::json!({"path": "/a"}), &ctx).await;
    assert!(again.is_ok() && again.untrusted.is_none());
    let w = fake.tool(FsToolKind::Write).unwrap();
    let out = w
        .call(serde_json::json!({"path": "/a", "content": "x"}), &ctx)
        .await;
    assert_eq!(out.status, ToolStatus::Cancelled);
    assert_eq!(fake.calls(FsToolKind::Read).len(), 2);
    assert_eq!(
        fake.calls(FsToolKind::Write)[0].holder,
        Holder::agent("s1", "delta")
    );
}
