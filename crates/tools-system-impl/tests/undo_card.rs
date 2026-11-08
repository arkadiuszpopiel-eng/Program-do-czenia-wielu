//! `system_env_set` zwraca kartę „Cofnij” (`UndoService::System` z krokiem dziennika zmiennych),
//! którą aplikacja cofa przez `SystemTools::undo_env`; odmowa nie daje karty.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{ctx, harness};
use serde_json::json;
use tools_common_contract::UndoService;

#[tokio::test]
async fn env_set_outcome_carries_system_undo_card() {
    let h = harness(true);
    let tool = h.tool("system_env_set");
    let ok = tool
        .call(json!({"name": "EDITOR", "value": "code"}), &ctx())
        .await;
    assert!(ok.is_ok(), "{}", ok.text);
    let undo = ok.undo.clone().expect("karta „Cofnij”");
    assert_eq!(undo.service, UndoService::System);
    assert_eq!(Some(undo.id), ok.data["undo_id"].as_u64());
    assert!(undo.text.contains("EDITOR"), "{}", undo.text);
    assert!(h.tools.undo_env(undo.id).is_ok(), "karta cofa zapis");

    let denied = tool
        .call(json!({"name": "LOCALAPPDATA", "value": "x"}), &ctx())
        .await;
    assert!(!denied.is_ok());
    assert!(denied.undo.is_none(), "odmowa bez karty");
}
