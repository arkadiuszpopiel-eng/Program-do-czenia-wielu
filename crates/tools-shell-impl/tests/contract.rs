//! Testy kontraktowe `tools-shell` na implementacji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

#[tokio::test]
async fn contract_suite_passes_on_impl() {
    let h = common::harness(&[("/Users/ala/Projekt/a.txt", "a")]);
    tools_shell_contract::contract_tests::run_all(&h.all(), common::WORK).await;
    assert!(h.exec.runs().is_empty(), "żadne polecenie nie wystartowało");
}
