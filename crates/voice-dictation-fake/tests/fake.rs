//! Atrapa przechodzi testy kontraktowe.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use voice_dictation_contract::contract_tests::{self, DesktopDriver};
use voice_dictation_fake::{FakeDictation, FakeWindows};

#[test]
fn contract_suite() {
    contract_tests::run_all(|| {
        let w = FakeWindows::default();
        (
            FakeDictation::new(w.clone()),
            Box::new(w) as Box<dyn DesktopDriver>,
        )
    });
}
