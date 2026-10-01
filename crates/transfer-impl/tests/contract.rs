//! Kontrakt współdzielony na prawdziwym kontenerze ZIP (katalogi tymczasowe).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use transfer_contract::contract_tests;

#[test]
fn contract_suite() {
    contract_tests::run_all(common::harness);
}
