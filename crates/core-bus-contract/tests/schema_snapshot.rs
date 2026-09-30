//! Snapshot schematu: wygenerowany JSON Schema musi być identyczny z plikiem w repo.
//! Aktualizacja: `UPDATE_SCHEMAS=1 cargo test -p core-bus-contract`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use core_bus_contract::{event_schema_json, EVENT_SCHEMA_VERSION};

fn schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/schemas")
        .join(format!("event.v{EVENT_SCHEMA_VERSION}.json"))
}

#[test]
fn schema_snapshot_matches_repo_file() {
    let generated = event_schema_json();
    let path = schema_path();
    if std::env::var_os("UPDATE_SCHEMAS").is_some() {
        std::fs::write(&path, &generated).expect("zapis schematu");
    }
    let on_disk = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "brak {}: {e}; uruchom UPDATE_SCHEMAS=1 cargo test",
            path.display()
        )
    });
    assert_eq!(
        on_disk,
        generated,
        "schemat w {} jest nieaktualny — uruchom UPDATE_SCHEMAS=1 cargo test -p core-bus-contract",
        path.display()
    );
}

#[test]
fn schema_is_valid_json_object_with_version() {
    let value: serde_json::Value = serde_json::from_str(&event_schema_json()).unwrap();
    assert_eq!(
        value["x-schema-version"],
        serde_json::json!(EVENT_SCHEMA_VERSION)
    );
    assert_eq!(value["title"], serde_json::json!("Event"));
    assert!(value["properties"]["kind"].is_object());
}
