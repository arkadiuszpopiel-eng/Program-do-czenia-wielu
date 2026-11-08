//! Regresja Q-1: katalog roboczy polecenia albo ścieżka w poleceniu prowadząca przez dowiązanie
//! do katalogu poświadczeń — odmowa przed startem procesu (sprawdzenie po rozwiązaniu dowiązań
//! przy każdym wywołaniu).

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::os::unix::fs::symlink;

use common::{ctx, harness};
use serde_json::json;
use tools_common_contract::{DenialReason, ToolStatus};

#[tokio::test]
async fn links_to_credentials_never_start_a_process() {
    let dir = std::env::temp_dir().join(format!("alfa-shell-links-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    std::fs::create_dir_all(dir.join(".ssh")).unwrap();
    symlink(dir.join(".ssh"), dir.join("roboczy")).unwrap();
    symlink(dir.join(".ssh"), dir.join("proj").join("klucze")).unwrap();
    let h = harness(&[]);
    let via_cwd = h
        .run()
        .call(
            json!({"command": "dir", "cwd": dir.join("roboczy").to_str().unwrap()}),
            &ctx(),
        )
        .await;
    let via_path = h
        .run()
        .call(
            json!({
                "command": format!("type {}", dir.join("proj/klucze/id_rsa").display()),
                "cwd": dir.join("proj").to_str().unwrap(),
            }),
            &ctx(),
        )
        .await;
    for out in [via_cwd, via_path] {
        assert!(
            matches!(
                out.status,
                ToolStatus::Denied {
                    reason: DenialReason::DenyList
                }
            ),
            "{out:?}"
        );
    }
    assert!(h.exec.runs().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
