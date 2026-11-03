//! Luka 3 z przeglądu planu 50 (3.11): usunięcie albo przeniesienie katalogu, w którym leży `.git`
//! (podrepozytorium), to dla Sędziego zmiana ścieżki chronionej — na „Automatycznie” pyta, choć
//! zwykły katalog idzie bez pytania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{ctx, harness};
use serde_json::json;
use tools_common_contract::ToolStatus;
use tools_fs_contract::FsToolKind as K;

const GIT: &str = "/Users/ala/Documents/lib/.git/config";
/// Sędzia zapytał (karta bez odpowiedzi do limitu) — nie „nie znaleziono” po wykonaniu.
fn asked(status: &ToolStatus) -> bool {
    !matches!(status, ToolStatus::Ok | ToolStatus::Failed { .. })
}

const PLAIN: &str = "/Users/ala/Documents/src/b.rs";

#[tokio::test]
async fn deleting_a_tree_with_git_asks_even_on_automatic() {
    let h = harness(&[(GIT, "[core]"), (PLAIN, "fn b() {}")]);
    // Atrapa FS nie usuwa katalogów domniemanych — zwykły zapis obok idzie bez pytania.
    let plain = h.call(K::Delete, json!({"path": "src/b.rs"})).await;
    assert!(plain.is_ok(), "{plain:?}");
    let mut c = ctx();
    c.approval_timeout = Duration::from_millis(50);
    let out = h.call_ctx(K::Delete, json!({"path": "lib"}), &c).await;
    assert!(asked(&out.status), "{out:?}");
    assert!(h.file(GIT).is_some(), "podrepozytorium zostało");
    let moved = h
        .call_ctx(K::Move, json!({"from": "lib", "to": "stare"}), &c)
        .await;
    assert!(asked(&moved.status), "{moved:?}");
    assert!(h.file(GIT).is_some());
}
