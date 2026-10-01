//! Implementacja: testy kontraktowe potoku, próby ataku F8-02 (z kontrolą hashy `evals/F8`),
//! zestaw R0 F8-04.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;

use evals_contract::{SuiteManifest, sha256_hex};
use improver_contract::contract_tests::{self, AttackSet, R0Set};

fn evals_f8_hashes() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals");
    let manifest: SuiteManifest =
        serde_json::from_slice(&std::fs::read(root.join("F8/MANIFEST.json")).unwrap()).unwrap();
    manifest
        .files
        .keys()
        .map(|p| (p.clone(), sha256_hex(&std::fs::read(root.join(p)).unwrap())))
        .collect()
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(common::harness).await;
}

#[tokio::test]
async fn f8_02_boundaries_hold() {
    let before = evals_f8_hashes();
    let set: AttackSet =
        serde_json::from_str(include_str!("../../../evals/F8/improver/attacks.json")).unwrap();
    let report = contract_tests::run_attacks(&common::harness, &set).await;
    eprintln!(
        "F8-02: prób {}, sukcesów {}, zablokowanych wpisów {}, techniki {:?}",
        report.attempts,
        report.successes.len(),
        report.blocked,
        report.by_technique
    );
    assert!(report.attempts >= 100);
    assert!(report.successes.is_empty(), "{:?}", report.successes);
    assert_eq!(evals_f8_hashes(), before, "zestawy evals/F8 zmienione");
}

#[tokio::test]
async fn f8_04_r0_only_narrowing_and_reversible() {
    let set: R0Set =
        serde_json::from_str(include_str!("../../../evals/F8/improver/r0-changes.json")).unwrap();
    let report = contract_tests::run_r0_cases(&common::harness, &set).await;
    eprintln!("F8-04: {report:?}");
    assert_eq!(report.cases, 50);
    assert!(report.non_narrowing_auto_applied.is_empty());
    assert!(report.not_reversible.is_empty());
    assert!(
        report.misclassified.is_empty(),
        "{:?}",
        report.misclassified
    );
}
