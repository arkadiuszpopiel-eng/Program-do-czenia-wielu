//! Runner scenariuszy `alfa.manifest` z zestawu `evals/F7/migrations/scenarios.json` (F7-08;
//! fala 5, m-06): manifest z fixture'u + modyfikacje (`set`, `append_file`) → `upcast_manifest`
//! i `Manifest::validate`. Oczekiwanie porównywane jako **podzbiór** (pola nieobecne w scenariuszu
//! nie są sprawdzane), jak opisuje `evals/F7/migrations/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use serde_json::Value;
use transfer_contract::migrate::upcast_manifest;
use transfer_contract::{Limits, Manifest, OLDEST_SCHEMA_VERSION, TransferError};

const SCENARIOS: &str = include_str!("../../../evals/F7/migrations/scenarios.json");

fn eval_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/F7/migrations")
}

/// Czy `expected` jest podzbiorem `actual` (obiekty — rekurencyjnie po kluczach, reszta — równość).
fn subset(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => e
            .iter()
            .all(|(k, v)| a.get(k).is_some_and(|x| subset(v, x))),
        _ => expected == actual,
    }
}

fn manifest_input(input: &Value) -> Value {
    let rel = input["manifest_from"].as_str().unwrap();
    let bytes = std::fs::read(eval_dir().join(rel)).unwrap();
    let mut manifest: Value = serde_json::from_slice(&bytes).unwrap();
    if let Some(set) = input.get("set").and_then(Value::as_object) {
        for (k, v) in set {
            manifest[k] = v.clone();
        }
    }
    if let Some(file) = input.get("append_file") {
        manifest["files"].as_array_mut().unwrap().push(file.clone());
    }
    manifest
}

#[test]
fn f7_manifest_scenarios() {
    let doc: Value = serde_json::from_str(SCENARIOS).unwrap();
    let mut ran = Vec::new();
    for s in doc["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["entity"] == "alfa.manifest")
    {
        let id = s["id"].as_str().unwrap();
        let expected = &s["expected"];
        let (outcome, steps) = match upcast_manifest(manifest_input(&s["input"])) {
            Ok((m, steps)) => (m.validate(&Limits::default()).map(|()| m), Some(steps)),
            Err(e) => (Err(e), None),
        };
        if expected["outcome"] == "ok" {
            assert!(outcome.is_ok(), "{id}: {outcome:?}");
        } else {
            let err = outcome.expect_err(id);
            let actual = serde_json::to_value(&err).unwrap();
            assert!(
                subset(&expected["error"], &actual),
                "{id}: jest {actual}, oczekiwano {}",
                expected["error"]
            );
        }
        if let Some(upcast) = expected.get("upcast") {
            let steps = serde_json::to_value(steps.unwrap_or_default()).unwrap();
            assert_eq!(&steps, upcast, "{id}");
        }
        ran.push(id.to_owned());
    }
    for must in ["m-04", "m-05", "m-06", "m-10"] {
        assert!(ran.iter().any(|i| i.starts_with(must)), "{must} ∉ {ran:?}");
    }
}

/// Manifest v1 z fixture'u F7 (poprawny, `schema_version` 1.0.0).
fn v1_manifest() -> Manifest {
    let bytes = std::fs::read(eval_dir().join("fixtures/alfa-v1/manifest.alfa.json")).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn older_major_is_rejected_as_older_not_newer() {
    // Fala 5, m-06: starsze major bez upcastera — odmowa „za stara”, bez rady „zaktualizuj”.
    let limits = Limits::default();
    assert_eq!(v1_manifest().validate(&limits), Ok(()));
    for old in [semver::Version::new(0, 9, 0), semver::Version::new(0, 0, 1)] {
        let mut m = v1_manifest();
        m.schema_version = old.clone();
        let err = m.validate(&limits).unwrap_err();
        assert_eq!(
            err,
            TransferError::OlderSchema {
                found: old.to_string(),
                oldest: OLDEST_SCHEMA_VERSION.to_owned(),
            }
        );
        assert!(!err.to_string().contains("zaktualizuj"), "{err}");
    }
    for newer in [semver::Version::new(2, 0, 0), semver::Version::new(1, 0, 1)] {
        let mut m = v1_manifest();
        m.schema_version = newer;
        let err = m.validate(&limits).unwrap_err();
        assert!(matches!(err, TransferError::NewerSchema { .. }), "{err:?}");
        assert!(err.to_string().contains("zaktualizuj Alfę"));
    }
    // Wydanie przedpremierowe obsługiwanej wersji (ten sam major) — przyjmowane jak dotąd.
    let mut pre = v1_manifest();
    pre.schema_version = semver::Version::parse("1.0.0-rc.1").unwrap();
    assert_eq!(pre.validate(&limits), Ok(()));
}

#[test]
fn oldest_schema_version_constants_agree() {
    assert_eq!(
        transfer_contract::oldest_schema_version().to_string(),
        OLDEST_SCHEMA_VERSION
    );
    assert!(transfer_contract::oldest_schema_version() <= transfer_contract::schema_version());
}
