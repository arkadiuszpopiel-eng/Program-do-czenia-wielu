//! Zestawy z repozytorium: F8 (ściśle — integralność wymagana), istniejące F2/F3/F5/F7 jako
//! przykłady formatu (adaptery bez zmiany treści; rozjazd hashy tylko raportowany, bo zestawy
//! są „proposed” i rozwijają je inne moduły).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use evals_contract::contract_tests::QualityRunner;
use evals_contract::{BootstrapConfig, EvalError, Split, SuiteCatalog, SuiteId, Variant};
use evals_impl::{DirCatalog, RunSpec, run_suite};

fn catalog() -> DirCatalog {
    DirCatalog::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals")).unwrap()
}

fn id(s: &str) -> SuiteId {
    SuiteId::new(s).unwrap()
}

#[test]
fn f8_suite_is_intact_and_listed() {
    let catalog = catalog();
    for p in catalog.problems() {
        eprintln!("problem manifestu: {p}");
    }
    assert!(
        catalog.problems().iter().all(|p| !p.starts_with("F8/")),
        "{:?}",
        catalog.problems()
    );
    let report = catalog.verify(&id("f8")).unwrap();
    eprintln!("F8 MANIFEST.json: digest {}", report.manifest_digest);
    assert!(report.is_intact(), "{report:?}");
    assert_eq!(report.checked, 3);
    assert_eq!(
        catalog.cases(&id("f8"), Split::Holdout),
        Err(EvalError::HoldoutSealed)
    );
}

#[tokio::test]
async fn existing_waves_are_format_examples() {
    let catalog = catalog();
    let listed: Vec<String> = catalog
        .suites()
        .iter()
        .map(|s| s.suite.to_string())
        .collect();
    for expected in [
        "f2-voice-samples",
        "f3-tools",
        "f5",
        "f7-recall-synthetic",
        "f8",
    ] {
        assert!(
            listed.iter().any(|l| l == expected),
            "{expected} ∉ {listed:?}"
        );
    }
    for s in catalog.suites() {
        let r = catalog.verify(&s.suite).unwrap();
        if !r.is_intact() {
            eprintln!(
                "rozjazd {} ({:?}): {:?} {:?}",
                s.suite, s.status, r.mismatched, r.missing
            );
        }
    }
    let f2 = catalog
        .cases(&id("f2-voice-samples"), Split::Dev)
        .unwrap_or_default()
        .len()
        + catalog
            .cases(&id("f2-voice-samples"), Split::Test)
            .unwrap_or_default()
            .len();
    assert!(f2 >= 1);
    let f3 = catalog
        .cases(&id("f3-tools"), Split::Test)
        .unwrap_or_default();
    assert!(f3.len() >= 30 && f3.iter().all(|c| c.class.is_some()));
    let f7 = catalog
        .cases(&id("f7-recall-synthetic"), Split::Test)
        .unwrap_or_default();
    assert!(f7.len() >= 200);
    let spec = RunSpec {
        suite: id("f3-tools"),
        split: Split::Test,
        repeats: 1,
        bootstrap: BootstrapConfig::default(),
        now_ms: 0,
    };
    let report = run_suite(
        &catalog,
        &spec,
        &QualityRunner::default(),
        &Variant::baseline(),
    )
    .await
    .unwrap();
    assert!(report.to_markdown().contains("`f3-tools`"));
    assert_eq!(report.aggregate.n_cases, f3.len());
}

/// Plan v6, zadanie C2.1: szkic zestawu po polsku jest w katalogu, zgodny z hashami manifestu,
/// a przypadki mają klasy i podziały (po jednym `dev` na plik, reszta `test`; bez `holdout`).
#[test]
fn v6_polish_core_suite_is_listed_and_intact() {
    let catalog = catalog();
    let suite = id("v6-pl-core");
    let report = catalog.verify(&suite).unwrap();
    assert!(report.is_intact(), "{report:?}");
    assert_eq!(report.checked, 3);
    let dev = catalog.cases(&suite, Split::Dev).unwrap();
    let test = catalog.cases(&suite, Split::Test).unwrap();
    assert_eq!((dev.len(), test.len()), (3, 33));
    let count = |class: &str| {
        test.iter()
            .chain(&dev)
            .filter(|c| c.class.as_deref() == Some(class))
            .count()
    };
    assert_eq!(count("tool_call"), 12);
    assert_eq!(count("rag_fidelity"), 12);
    assert_eq!(count("benign_control"), 2);
    for case in test.iter().chain(&dev) {
        assert!(case.class.is_some(), "{} bez klasy", case.id);
        assert!(
            case.input["messages"].is_array(),
            "{} bez wiadomości",
            case.id
        );
        assert!(case.expected["note"].is_string(), "{} bez opisu", case.id);
    }
    // Kanarek nie wygląda jak prawdziwy klucz i nie pojawia się w żadnym oczekiwanym tekście
    // inaczej niż jako zakaz.
    for case in test.iter().filter(|c| c.input["secrets"].is_object()) {
        let canary = case.input["secrets"]["canary"].as_str().unwrap();
        assert!(canary.starts_with("ALFA-KANARKOWY"), "{}", case.id);
    }
}

/// Fala 5 (pkt 3): `alfa-evals verify evals` na katalogu z repozytorium kończy się kodem 0 —
/// żaden manifest nie jest powtórzony ani niepoprawny (np. `F5/voice` nie może udawać zestawu
/// `f5`), a zamrożone zestawy są zgodne z hashami. Pilnuje CI przed cichym rozjazdem katalogu.
#[test]
fn alfa_evals_verify_passes_on_repo_catalog() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals");
    let catalog = DirCatalog::open(&root).unwrap();
    assert!(
        catalog.problems().is_empty(),
        "problemy katalogu: {:?}",
        catalog.problems()
    );
    let listed: Vec<String> = catalog
        .suites()
        .iter()
        .map(|s| s.suite.to_string())
        .collect();
    assert!(
        listed.iter().any(|l| l == "f5") && listed.iter().any(|l| l == "f5-voice"),
        "{listed:?}"
    );
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_alfa-evals"))
        .arg("verify")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "alfa-evals verify: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
