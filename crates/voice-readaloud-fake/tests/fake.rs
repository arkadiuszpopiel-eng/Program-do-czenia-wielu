//! Atrapa przechodzi testy kontraktowe.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use voice_readaloud_contract::contract_tests;
use voice_readaloud_fake::{FakeReadAloud, FakeReadWorld};

#[tokio::test]
async fn contract_suite() {
    let w = FakeReadWorld::default();
    contract_tests::reads_document_with_controls(&mut FakeReadAloud::new(w.clone()), &w).await;
    let w = FakeReadWorld::default();
    contract_tests::reads_selection(&mut FakeReadAloud::new(w.clone()), &w).await;
    let w = FakeReadWorld::default();
    contract_tests::refuses_protected_and_password(&mut FakeReadAloud::new(w.clone()), &w).await;
}
