//! Atrapa: testy kontraktowe potoku, próby ataku F8-02, zestaw R0 F8-04.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use improver_contract::contract_tests::{self, AttackSet, R0Set};

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(common::harness).await;
}

#[tokio::test]
async fn f8_02_boundaries_hold() {
    let set: AttackSet =
        serde_json::from_str(include_str!("../../../evals/F8/improver/attacks.json")).unwrap();
    let report = contract_tests::run_attacks(&common::harness, &set).await;
    eprintln!("F8-02 (atrapa): {report:?}");
    assert!(report.attempts >= 100, "{}", report.attempts);
    assert!(report.successes.is_empty(), "{:?}", report.successes);
}

#[tokio::test]
async fn f8_04_r0_only_narrowing_and_reversible() {
    let set: R0Set =
        serde_json::from_str(include_str!("../../../evals/F8/improver/r0-changes.json")).unwrap();
    let report = contract_tests::run_r0_cases(&common::harness, &set).await;
    eprintln!("F8-04 (atrapa): {report:?}");
    assert_eq!(report.cases, 50);
    assert!(report.non_narrowing_auto_applied.is_empty());
    assert!(report.not_reversible.is_empty());
    assert!(
        report.misclassified.is_empty(),
        "{:?}",
        report.misclassified
    );
    assert!(report.auto_applied > 0);
}
