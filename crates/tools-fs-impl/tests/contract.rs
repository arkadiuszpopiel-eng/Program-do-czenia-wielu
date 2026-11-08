//! Testy kontraktowe `tools-fs` na implementacji (wirtualny FS + Broker z prawdziwym silnikiem).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

#[tokio::test]
async fn contract_suite_passes_on_impl() {
    let h = common::harness(&[("/Users/ala/Documents/kontrakt.txt", "treść")]);
    tools_fs_contract::contract_tests::run_all(&h.all(), "/Users/ala/Documents").await;
    assert!(
        h.journal.steps(&"s1".into()).is_empty(),
        "żadnej mutacji w testach kontraktowych"
    );
}
