//! Limit kopii zakresu jest błędem wykonania, a nie odmową uprawnień Brokera.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

#[tokio::test]
async fn oversized_snapshot_does_not_claim_broker_denial_or_execute_command() {
    let names: Vec<String> = (0..10_001)
        .map(|i| format!("/Users/ala/Projekt/f{i}.txt"))
        .collect();
    let files: Vec<(&str, &str)> = names.iter().map(|p| (p.as_str(), "x")).collect();
    let h = common::harness(&files);
    let out = h.sh("Get-ChildItem").await;
    assert!(
        matches!(out.status, tools_common_contract::ToolStatus::Failed { .. }),
        "{out:?}"
    );
    assert!(
        out.text.contains("fs_write") && !out.text.contains("odmowa"),
        "{}",
        out.text
    );
    assert!(h.exec.runs().is_empty());
}
