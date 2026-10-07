//! Kontrakt F7 (`contract_tests_f7`) i kontrakt v0 (fasada) na silniku nad prawdziwymi bazami
//! SQLCipher (sesje + zakresy własne) z indeksem atrapy `FakeSearch`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use memory_contract::{contract_tests, contract_tests_f7};

#[test]
fn contract_suite_f7_on_sqlite() {
    contract_tests_f7::run_all(common::stack_with);
}

#[test]
fn contract_suite_v0_on_f7_engine() {
    contract_tests::run_all(common::stack);
}
