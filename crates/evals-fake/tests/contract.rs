//! Atrapa przechodzi współdzielone testy kontraktowe bramki i katalogu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use evals_contract::contract_tests::{self, GateScenario, SuiteFixture};
use evals_contract::{EvalGate, SuiteCatalog};
use evals_fake::{FakeCatalog, FakeGate};

fn catalog(fixtures: Vec<SuiteFixture>) -> Arc<FakeCatalog> {
    let catalog = Arc::new(FakeCatalog::new());
    for f in fixtures {
        let _ = catalog.add_suite(f.manifest, f.files);
    }
    catalog
}

#[test]
fn catalog_contract() {
    contract_tests::run_catalog_suite(|fixtures| catalog(fixtures) as Arc<dyn SuiteCatalog>);
}

#[tokio::test]
async fn gate_contract() {
    contract_tests::run_gate_suite(|sc: GateScenario| async move {
        let public = catalog(vec![sc.public]);
        let gate = FakeGate::new(public, sc.runner, sc.policy)
            .with_holdout(sc.holdout.manifest, sc.holdout.files)
            .unwrap();
        Arc::new(gate) as Arc<dyn EvalGate>
    })
    .await;
}

#[test]
fn holdout_manifest_must_be_holdout_only() {
    let all = contract_tests::cases("t", evals_contract::Split::Test, 3, "x");
    let fixture = contract_tests::build_suite(
        "s",
        "s",
        evals_contract::SuiteStatus::Frozen,
        &all,
        Vec::new(),
    );
    let gate = FakeGate::new(
        Arc::new(FakeCatalog::new()),
        Arc::new(contract_tests::QualityRunner::default()),
        evals_contract::GatePolicy::default(),
    );
    assert!(gate.with_holdout(fixture.manifest, fixture.files).is_err());
}
