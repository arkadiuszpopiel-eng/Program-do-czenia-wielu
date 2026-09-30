//! Kontrakt współdzielony (`ACC-F1-search-01..03`) na prawdziwych bazach SQLCipher.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use search_contract::contract_tests;

#[test]
fn contract_suite() {
    contract_tests::run_all(common::harness);
}
