//! Atrapa przechodzi testy kontraktowe; zapis i cofnięcie z konfliktem.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use safety_broker_contract::{Holder, TaintSource};
use tools_clipboard_contract::{ClipboardUndo, ClipboardUndoError};
use tools_clipboard_fake::FakeClipboardTools;
use tools_common_contract::{ToolCtx, Toolset};

#[tokio::test]
async fn contract_suite_passes_on_fake() {
    let fake = FakeClipboardTools::with_text("x");
    tools_clipboard_contract::contract_tests::run_all(&fake.tools()).await;
    assert_eq!(fake.text().as_deref(), Some("x"));
}

#[tokio::test]
async fn read_write_undo() {
    let fake = FakeClipboardTools::with_text("z zewnątrz");
    let ctx = ToolCtx::new(Holder::agent("s1", "delta"));
    let tools = fake.tools();
    let read = tools[0].call(serde_json::json!({}), &ctx).await;
    assert_eq!(read.untrusted, Some(TaintSource::Screen));
    let w = tools[1]
        .call(serde_json::json!({"text": "nowe"}), &ctx)
        .await;
    let id = w.undo.unwrap().id;
    fake.undo(id).unwrap();
    assert_eq!(fake.text().as_deref(), Some("z zewnątrz"));
    let w2 = tools[1]
        .call(serde_json::json!({"text": "a"}), &ctx)
        .await
        .undo
        .unwrap()
        .id;
    fake.set_external("b");
    assert_eq!(fake.undo(w2), Err(ClipboardUndoError::Conflict));
    assert_eq!(fake.undo(99), Err(ClipboardUndoError::Unknown(99)));
    assert_eq!(fake.counts(), (1, 2));
}
