//! Zapisany schemat łącza (`packages/schemas/ipc.v1.json`) musi odpowiadać typom Rust (ADR 0013:
//! pliki generowane są pod kontrolą `git diff`). Po zmianie DTO albo listy komend:
//! `UPDATE_SCHEMAS=1 cargo test -p app-api --test ipc_schema`, potem
//! `node scripts/gen-ipc-types.mjs` (typy TypeScript).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

fn schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/schemas/ipc.v1.json")
}

#[test]
fn saved_schema_matches_rust_types() {
    let current = format!(
        "{}\n",
        serde_json::to_string_pretty(&app_api::schema::ipc_schema()).unwrap()
    );
    let path = schema_path();
    if std::env::var("UPDATE_SCHEMAS").is_ok_and(|v| v == "1") {
        std::fs::write(&path, &current).unwrap();
        return;
    }
    let saved = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        saved == current,
        "packages/schemas/ipc.v1.json nie odpowiada typom Rust — uruchom:\n  \
         UPDATE_SCHEMAS=1 cargo test -p app-api --test ipc_schema\n  \
         node scripts/gen-ipc-types.mjs"
    );
}

#[test]
fn channel_commands_are_listed_once() {
    for c in app_api::CHANNEL_COMMANDS {
        assert_eq!(app_api::COMMANDS.iter().filter(|x| *x == c).count(), 1);
    }
    assert_eq!(app_api::COMMANDS.len(), 286);
}

#[test]
fn paths_under_root_follow_layout() {
    let p = app_api::AppPaths::under(std::path::Path::new("/x"));
    assert_eq!(p.sessions(), std::path::Path::new("/x/local/sessions"));
    assert_eq!(
        p.webview_data(),
        std::path::Path::new("/x/local/webview-data")
    );
    assert_eq!(p.workdirs(), std::path::Path::new("/x/user/Sesje"));
}

#[test]
fn tool_status_retains_legacy_wire_values_and_confirmation() {
    for value in ["running", "done", "error", "needs_confirmation"] {
        let status: app_api::dto::ToolStatus =
            serde_json::from_value(serde_json::json!(value)).unwrap();
        assert_eq!(serde_json::to_value(status).unwrap(), value);
    }
}
