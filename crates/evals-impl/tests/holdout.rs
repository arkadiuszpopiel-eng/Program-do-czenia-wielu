//! Holdout zapieczętowany (ACCEPTANCE F8-03): katalog publiczny nie czyta `holdout/` ani `corpus/`
//! (także przez dowiązanie, `..` i inną wielkość liter), werdykt i zdarzenia bez danych przypadków.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use core_bus_fake::FakeBus;
use evals_contract::contract_tests::{
    self, HOLDOUT_MARKER, QualityRunner, SUITE, build_suite, cases,
};
use evals_contract::{
    EVENT_GATE_DECIDED, EVENT_INTEGRITY_FAILED, EvalError, EvalGate, GatePolicy, GateRequest,
    GateStage, ManualClock, Split, SuiteCatalog, SuiteId, SuiteStatus, Variant, event_kind,
};
use evals_impl::{DirCatalog, HoldoutGate};
use serde_json::json;

use common::write_fixture;

fn request(stage: GateStage, q: f64) -> GateRequest {
    GateRequest {
        suite: SuiteId::new(SUITE).unwrap(),
        stage,
        baseline: Variant::baseline(),
        candidate: Variant {
            id: "k".into(),
            patch: BTreeMap::from([("quality".to_owned(), json!(q))]),
        },
        repeats: 5,
        primary_metric: None,
    }
}

#[test]
fn catalog_never_reads_sealed_directories() {
    let root = tempfile::tempdir().unwrap();
    // Holdout i korpus z poprawnymi manifestami pod korzeniem publicznym — niewidoczne.
    let hold = build_suite(
        "ukryty",
        "holdout/h",
        SuiteStatus::Frozen,
        &cases("h", Split::Test, 3, "X"),
        Vec::new(),
    );
    let sub = root.path().join("holdout");
    std::fs::create_dir_all(&sub).unwrap();
    write_fixture(root.path(), &hold);
    std::fs::rename(
        root.path().join("ukryty.suite.json"),
        sub.join("ukryty.suite.json"),
    )
    .unwrap();
    let corpus = build_suite(
        "korpus",
        "corpus/c",
        SuiteStatus::Proposed,
        &cases("c", Split::Dev, 3, "X"),
        Vec::new(),
    );
    write_fixture(root.path(), &corpus);
    // Zestaw publiczny, który próbuje wskazać pliki w katalogach zapieczętowanych.
    let mut sneaky = build_suite(
        "podstep",
        "jawny",
        SuiteStatus::Proposed,
        &cases("p", Split::Test, 3, "X"),
        Vec::new(),
    );
    let hash = sneaky.manifest.files.values().next().cloned().unwrap();
    sneaky
        .manifest
        .files
        .insert("Holdout/h/test.ndjson".into(), hash);
    write_fixture(root.path(), &sneaky);

    let catalog = DirCatalog::open(root.path()).unwrap();
    let ids: Vec<String> = catalog
        .suites()
        .iter()
        .map(|s| s.suite.to_string())
        .collect();
    assert!(ids.is_empty(), "{ids:?}");
    assert!(!ids.contains(&"ukryty".to_owned()));
    assert!(!ids.contains(&"podstep".to_owned()));
    assert!(catalog.problems().iter().any(|p| p.contains("podstep")));
    // Korpus: manifest leży w korzeniu, ale wskazuje `corpus/…` → odrzucony jako zapieczętowany.
    assert!(!ids.contains(&"korpus".to_owned()));
    assert_eq!(
        catalog.cases(&SuiteId::new("ukryty").unwrap(), Split::Holdout),
        Err(EvalError::HoldoutSealed)
    );
}

#[cfg(unix)]
#[test]
fn symlink_escape_is_rejected() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("tajne.ndjson"),
        b"{\"id\":\"x\",\"split\":\"test\"}\n",
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut fx = build_suite(
        "dowiazanie",
        "d",
        SuiteStatus::Proposed,
        &cases("d", Split::Test, 2, "X"),
        Vec::new(),
    );
    write_fixture(root.path(), &fx);
    std::os::unix::fs::symlink(
        outside.path().join("tajne.ndjson"),
        root.path().join("d/link.ndjson"),
    )
    .unwrap();
    let hash = evals_contract::sha256_hex(b"{\"id\":\"x\",\"split\":\"test\"}\n");
    fx.manifest.files.insert("d/link.ndjson".into(), hash);
    fx.manifest.cases[0].path = "d/link.ndjson".into();
    std::fs::write(
        root.path().join("dowiazanie.suite.json"),
        serde_json::to_vec(&fx.manifest).unwrap(),
    )
    .unwrap();
    let catalog = DirCatalog::open(root.path()).unwrap();
    let id = SuiteId::new("dowiazanie").unwrap();
    assert!(!catalog.verify(&id).unwrap().is_intact());
    assert!(catalog.cases(&id, Split::Test).is_err());
}

#[tokio::test]
async fn verdict_and_events_carry_no_holdout_data() {
    let sc = contract_tests::standard_scenario(GatePolicy::default());
    let (public, holdout) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    write_fixture(public.path(), &sc.public);
    write_fixture(holdout.path(), &sc.holdout);
    let bus = FakeBus::default();
    let catalog = Arc::new(DirCatalog::open(public.path()).unwrap());
    let gate = HoldoutGate::open(
        holdout.path(),
        catalog.clone(),
        Arc::new(QualityRunner::default()),
        GatePolicy::default(),
        Arc::new(ManualClock::new(0)),
    )
    .unwrap()
    .with_bus(Arc::new(bus.clone()));
    assert_eq!(gate.holdout_suites(), [SuiteId::new(SUITE).unwrap()]);
    // Katalog publiczny nie zna przypadków holdoutu.
    assert_eq!(
        catalog.cases(&SuiteId::new(SUITE).unwrap(), Split::Holdout),
        Err(EvalError::HoldoutSealed)
    );
    let v = gate
        .evaluate(request(GateStage::Holdout, 0.9))
        .await
        .unwrap();
    assert!(v.passed());
    let events = bus.recorded_of_kind(&event_kind(EVENT_GATE_DECIDED));
    assert_eq!(events.len(), 1);
    let text = serde_json::to_string(&events[0].payload).unwrap();
    assert!(
        !text.contains(HOLDOUT_MARKER) && !text.contains("klasa"),
        "{text}"
    );

    // Zmiana pliku holdoutu po otwarciu bramki → błąd i zdarzenie integralności (bez ścieżek).
    let path = holdout.path().join(&sc.holdout.manifest.cases[0].path);
    std::fs::write(&path, b"").unwrap();
    let err = gate.evaluate(request(GateStage::Holdout, 0.9)).await;
    assert!(matches!(err, Err(EvalError::IntegrityViolation { .. })));
    let failed = bus.recorded_of_kind(&event_kind(EVENT_INTEGRITY_FAILED));
    assert_eq!(failed.len(), 1);
    assert!(!failed[0].payload.to_string().contains(HOLDOUT_MARKER));
}

#[test]
fn gate_refuses_weak_policy_and_public_manifest_as_holdout() {
    let sc = contract_tests::standard_scenario(GatePolicy::default());
    let dir = tempfile::tempdir().unwrap();
    write_fixture(dir.path(), &sc.holdout);
    let catalog = Arc::new(DirCatalog::open(dir.path()).unwrap()) as Arc<dyn SuiteCatalog>;
    let weak = GatePolicy {
        min_repeats: 1,
        ..GatePolicy::default()
    };
    let open = |p: GatePolicy, root: &std::path::Path| {
        HoldoutGate::open(
            root,
            catalog.clone(),
            Arc::new(QualityRunner::default()),
            p,
            Arc::new(ManualClock::new(0)),
        )
    };
    assert!(matches!(
        open(weak, dir.path()),
        Err(EvalError::InvalidPolicy(_))
    ));
    let public_dir = tempfile::tempdir().unwrap();
    write_fixture(public_dir.path(), &sc.public);
    assert!(open(GatePolicy::default(), public_dir.path()).is_err());
}
