//! Współdzielony test kontraktowy na implementacji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use accounts_hub_contract::contract_tests;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|catalog| async move { common::hub(catalog) }).await;
}
