//! Regresja Q-1: dowiązanie (symlink/junction) w katalogu roboczym albo katalog roboczy będący
//! dowiązaniem do katalogu poświadczeń nie omija deny-listy — ścieżka sprawdzana jest też po
//! rozwiązaniu dowiązań, przy każdym wywołaniu narzędzia (dowiązanie podmienione po wyborze
//! katalogu). Zerwane dowiązanie = odmowa (cel nieustalony).

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::os::unix::fs::symlink;
use std::path::PathBuf;

use common::harness;
use safety_broker_contract::Holder;
use serde_json::json;
use tools_common_contract::{DenialReason, ToolCtx, ToolStatus};
use tools_fs_contract::FsToolKind as K;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("alfa-fs-links-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    std::fs::create_dir_all(dir.join(".ssh")).unwrap();
    std::fs::write(dir.join(".ssh").join("id_rsa"), b"klucz prywatny").unwrap();
    dir
}

fn ctx(workdir: &std::path::Path) -> ToolCtx {
    ToolCtx::new(Holder::agent("s1", "delta")).with_workdir(workdir.to_str().unwrap())
}

fn denied(status: &ToolStatus) -> bool {
    matches!(
        status,
        ToolStatus::Denied {
            reason: DenialReason::DenyList
        }
    )
}

#[tokio::test]
async fn links_to_credentials_are_denied_at_call_time() {
    let dir = scratch("call");
    let proj = dir.join("proj");
    let h = harness(&[]);
    // Dowiązanie wewnątrz katalogu roboczego.
    symlink(dir.join(".ssh"), proj.join("klucze")).unwrap();
    let out = h
        .call_ctx(K::Read, json!({"path": "klucze/id_rsa"}), &ctx(&proj))
        .await;
    assert!(denied(&out.status), "{out:?}");
    // Katalog roboczy sam jest dowiązaniem (np. podmienionym po wyborze).
    let swapped = dir.join("roboczy");
    symlink(dir.join(".ssh"), &swapped).unwrap();
    let out = h
        .call_ctx(K::Read, json!({"path": "id_rsa"}), &ctx(&swapped))
        .await;
    assert!(denied(&out.status), "{out:?}");
    let out = h
        .call_ctx(K::List, json!({"path": "."}), &ctx(&swapped))
        .await;
    assert!(denied(&out.status), "{out:?}");
    // Zerwane dowiązanie: celu nie da się ustalić.
    symlink(dir.join("nie-ma"), proj.join("zerwane")).unwrap();
    let out = h
        .call_ctx(
            K::Write,
            json!({"path": "zerwane", "content": "x"}),
            &ctx(&proj),
        )
        .await;
    assert!(denied(&out.status), "{out:?}");
    assert!(!dir.join("nie-ma").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
