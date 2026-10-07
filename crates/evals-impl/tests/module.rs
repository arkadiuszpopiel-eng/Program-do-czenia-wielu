//! Moduł rejestru: manifest, cykl życia, zdrowie (zamrożony zestaw zmieniony → niezdrowy),
//! przebieg z raportem JSON/Markdown.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use evals_contract::contract_tests::{QualityRunner, build_suite, cases};
use evals_contract::{
    BootstrapConfig, CompareVerdict, EVENT_RUN_COMPLETED, EvalError, Split, SuiteId, SuiteStatus,
    Variant, event_kind,
};
use evals_impl::{DirCatalog, EvalsModule, MODULE_TOML, RunSpec, compare_suite, run_suite};
use serde_json::json;

use common::write_fixture;

#[tokio::test]
async fn manifest_lifecycle_and_health() {
    let dir = tempfile::tempdir().unwrap();
    let mut all = cases("d", Split::Dev, 4, "x");
    all.extend(cases("t", Split::Test, 20, "x"));
    let fx = build_suite("zamrozony", "z", SuiteStatus::Frozen, &all, Vec::new());
    write_fixture(dir.path(), &fx);
    let catalog = Arc::new(DirCatalog::open(dir.path()).unwrap());
    let mut module = EvalsModule::new(catalog.clone()).unwrap();
    let m = module.manifest();
    assert_eq!(m.id.as_str(), "evals");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.lifecycle, Lifecycle::OnDemand);
    assert!(MODULE_TOML.contains("evals-contract@1"));
    assert_eq!(module.health(), HealthStatus::NotStarted);
    assert_eq!(module.stop().await, Err(ModuleError::NotStarted));

    let bus = FakeBus::default();
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx.clone()).await.unwrap();
    assert_eq!(module.start(ctx).await, Err(ModuleError::AlreadyStarted));
    assert_eq!(module.health(), HealthStatus::Healthy);

    let spec = RunSpec {
        suite: SuiteId::new("zamrozony").unwrap(),
        split: Split::Test,
        repeats: 2,
        bootstrap: BootstrapConfig::default(),
        now_ms: 1,
    };
    let runner = QualityRunner::default();
    let good = Variant {
        id: "dobry".into(),
        patch: BTreeMap::from([("quality".to_owned(), json!(1.0))]),
    };
    let report = run_suite(catalog.as_ref(), &spec, &runner, &good)
        .await
        .unwrap();
    assert_eq!(report.aggregate.n_cases, 20);
    assert!(report.integrity.as_ref().unwrap().is_intact());
    assert!(report.to_markdown().contains("wariant `dobry`"));
    module.publish_report(&report).await;
    assert_eq!(
        bus.recorded_of_kind(&event_kind(EVENT_RUN_COMPLETED)).len(),
        1
    );

    let bad = Variant {
        id: "slaby".into(),
        patch: BTreeMap::from([("quality".to_owned(), json!(0.0))]),
    };
    let cmp = compare_suite(catalog.as_ref(), &spec, &runner, &bad, &good)
        .await
        .unwrap();
    assert_eq!(cmp.comparison.unwrap().verdict, CompareVerdict::Better);
    let holdout = RunSpec {
        split: Split::Holdout,
        ..spec.clone()
    };
    assert_eq!(
        run_suite(catalog.as_ref(), &holdout, &runner, &good)
            .await
            .unwrap_err(),
        EvalError::HoldoutSealed
    );

    // Zmiana pliku zamrożonego zestawu: zdrowie „niezdrowy”, przebieg odmówiony.
    std::fs::write(dir.path().join(&fx.manifest.cases[0].path), b"\n").unwrap();
    assert!(matches!(module.health(), HealthStatus::Unhealthy(_)));
    assert!(matches!(
        run_suite(catalog.as_ref(), &spec, &runner, &good).await,
        Err(EvalError::IntegrityViolation { .. })
    ));
    module.stop().await.unwrap();
}
