//! Implementacja przechodzi współdzielone testy kontraktowe (pliki w katalogu tymczasowym).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use evals_contract::contract_tests::{self, GateScenario};
use evals_contract::{
    EvalCase, EvalError, EvalGate, GatePolicy, GateRequest, GateVerdict, IntegrityReport,
    ManualClock, Split, SuiteCatalog, SuiteId, SuiteInfo, SuiteManifest,
};
use evals_impl::{DirCatalog, HoldoutGate};
use tempfile::TempDir;

use common::write_fixture;

/// Katalog trzymający katalog tymczasowy przy życiu.
struct OwnedCatalog {
    _dir: TempDir,
    inner: DirCatalog,
}

impl SuiteCatalog for OwnedCatalog {
    fn suites(&self) -> Vec<SuiteInfo> {
        self.inner.suites()
    }
    fn manifest(&self, suite: &SuiteId) -> Result<SuiteManifest, EvalError> {
        self.inner.manifest(suite)
    }
    fn verify(&self, suite: &SuiteId) -> Result<IntegrityReport, EvalError> {
        self.inner.verify(suite)
    }
    fn cases(&self, suite: &SuiteId, split: Split) -> Result<Vec<EvalCase>, EvalError> {
        self.inner.cases(suite, split)
    }
}

struct OwnedGate {
    _dirs: (TempDir, TempDir),
    inner: HoldoutGate,
}

#[async_trait]
impl EvalGate for OwnedGate {
    async fn evaluate(&self, request: GateRequest) -> Result<GateVerdict, EvalError> {
        self.inner.evaluate(request).await
    }
    fn policy(&self) -> GatePolicy {
        self.inner.policy()
    }
}

#[test]
fn catalog_contract() {
    contract_tests::run_catalog_suite(|fixtures| {
        let dir = tempfile::tempdir().unwrap();
        for f in &fixtures {
            write_fixture(dir.path(), f);
        }
        let inner = DirCatalog::open(dir.path()).unwrap();
        Arc::new(OwnedCatalog { _dir: dir, inner }) as Arc<dyn SuiteCatalog>
    });
}

#[tokio::test]
async fn gate_contract() {
    contract_tests::run_gate_suite(|sc: GateScenario| async move {
        let public = tempfile::tempdir().unwrap();
        let holdout = tempfile::tempdir().unwrap();
        write_fixture(public.path(), &sc.public);
        write_fixture(holdout.path(), &sc.holdout);
        let catalog = Arc::new(DirCatalog::open(public.path()).unwrap());
        let inner = HoldoutGate::open(
            holdout.path(),
            catalog,
            sc.runner,
            sc.policy,
            Arc::new(ManualClock::new(0)),
        )
        .unwrap();
        Arc::new(OwnedGate {
            _dirs: (public, holdout),
            inner,
        }) as Arc<dyn EvalGate>
    })
    .await;
}
