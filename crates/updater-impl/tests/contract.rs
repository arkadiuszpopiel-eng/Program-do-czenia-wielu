//! Kontrakt współdzielony na systemie plików (katalog tymczasowy, prawdziwe podpisy minisign).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

#[test]
fn contract_suite() {
    updater_contract::contract_tests::run_all(common::harness);
}
