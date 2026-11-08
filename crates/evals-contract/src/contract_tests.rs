//! Współdzielone testy kontraktowe (feature `contract-tests`) — bramka ([`EvalGate`]) i katalog
//! ([`SuiteCatalog`]) uruchamiane na `evals-impl` (pliki w katalogu tymczasowym) i `evals-fake`.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;

use crate::{
    CandidateRunner, CaseFormat, CaseOutcome, CaseSource, EvalCase, EvalError, EvalGate,
    GatePolicy, GateRequest, GateStage, MANIFEST_SCHEMA_VERSION, Split, SuiteCatalog, SuiteId,
    SuiteManifest, SuiteStatus, Threshold, Variant, sha256_hex,
};

/// Zestaw do testów: manifest i treść plików (ścieżka względem korzenia magazynu → bajty).
#[derive(Debug, Clone)]
pub struct SuiteFixture {
    /// Manifest.
    pub manifest: SuiteManifest,
    /// Pliki.
    pub files: BTreeMap<String, Vec<u8>>,
}

impl SuiteFixture {
    /// Podmienia treść pliku bez aktualizacji manifestu (symulacja zmiany zestawu).
    #[must_use]
    pub fn tampered(mut self, path: &str, bytes: &[u8]) -> Self {
        self.files.insert(path.to_owned(), bytes.to_vec());
        self
    }
}

/// Buduje zestaw w formacie natywnym: przypadki zapisane jako NDJSON `<dir>/<split>.ndjson`.
pub fn build_suite(
    suite: &str,
    dir: &str,
    status: SuiteStatus,
    cases: &[EvalCase],
    thresholds: Vec<Threshold>,
) -> SuiteFixture {
    let mut by_split: BTreeMap<Split, String> = BTreeMap::new();
    for case in cases {
        let line = serde_json::to_string(case).unwrap_or_default();
        let text = by_split.entry(case.split).or_default();
        text.push_str(&line);
        text.push('\n');
    }
    let mut files = BTreeMap::new();
    let mut sources = Vec::new();
    for (split, text) in by_split {
        let path = format!("{dir}/{}.ndjson", split.as_str());
        sources.push(CaseSource {
            path: path.clone(),
            format: CaseFormat::EvalCases,
            split: Some(split),
        });
        files.insert(path, text.into_bytes());
    }
    let manifest = SuiteManifest {
        schema: MANIFEST_SCHEMA_VERSION,
        suite: SuiteId::new(suite).unwrap_or_else(|e| panic!("{e}")),
        wave: "F8".into(),
        version: 1,
        status,
        created: "2026-10-01".into(),
        accepted_by: (status == SuiteStatus::Frozen).then(|| "test".to_owned()),
        description: "zestaw testowy kontraktu".into(),
        files: files
            .iter()
            .map(|(p, b)| (p.clone(), sha256_hex(b)))
            .collect(),
        cases: sources,
        thresholds,
    };
    SuiteFixture { manifest, files }
}

/// Przypadki `n` w podziale z prefiksem identyfikatora i znacznikiem treści.
pub fn cases(prefix: &str, split: Split, n: usize, marker: &str) -> Vec<EvalCase> {
    (0..n)
        .map(|i| EvalCase {
            id: format!("{prefix}-{i}"),
            split,
            class: Some(format!("{marker}-klasa-{}", i % 2)),
            input: json!({ "tekst": format!("{marker}-wejscie-{i}") }),
            expected: json!({ "odpowiedz": format!("{marker}-oczekiwane-{i}") }),
        })
        .collect()
}

/// Deterministyczny „system w piaskownicy”: jakość wariantu z łatki `quality` (0..1),
/// błąd uruchomienia przy `fail_runner = true`; zapamiętuje widziane przypadki.
#[derive(Debug, Default)]
pub struct QualityRunner {
    seen: Mutex<Vec<String>>,
}

impl QualityRunner {
    /// Identyfikatory przypadków przekazanych do uruchomienia.
    pub fn seen(&self) -> Vec<String> {
        self.seen.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl CandidateRunner for QualityRunner {
    async fn run(
        &self,
        variant: &Variant,
        case: &EvalCase,
        repeat: u32,
    ) -> Result<CaseOutcome, String> {
        if let Ok(mut g) = self.seen.lock() {
            g.push(case.id.clone());
        }
        if variant.patch.get("fail_runner") == Some(&json!(true)) {
            return Err("piaskownica nie wystartowała".into());
        }
        let quality = variant
            .patch
            .get("quality")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.5);
        let digest = sha256_hex(format!("{}#{repeat}", case.id).as_bytes());
        let bucket = u32::from_str_radix(&digest[..4], 16).unwrap_or(0);
        let passed = f64::from(bucket) / 65536.0 < quality;
        Ok(CaseOutcome::new(case, repeat, passed).with_metric("quality", quality))
    }
}

/// Scenariusz bramki: zestaw publiczny (dev/test) i holdout o tym samym identyfikatorze.
pub struct GateScenario {
    /// Zestaw publiczny (korzeń `evals/`).
    pub public: SuiteFixture,
    /// Holdout (osobny korzeń, poza gitem).
    pub holdout: SuiteFixture,
    /// Runner Jądra.
    pub runner: Arc<QualityRunner>,
    /// Polityka.
    pub policy: GatePolicy,
}

/// Identyfikator zestawu scenariusza.
pub const SUITE: &str = "kontrakt";
/// Znacznik treści holdoutu (nie może pojawić się w werdykcie).
pub const HOLDOUT_MARKER: &str = "SEKRET";

/// Scenariusz standardowy: 12 dev + 12 test publicznie, 20 przypadków holdoutu.
pub fn standard_scenario(policy: GatePolicy) -> GateScenario {
    let mut public_cases = cases("dev", Split::Dev, 12, "jawne");
    public_cases.extend(cases("test", Split::Test, 12, "jawne"));
    GateScenario {
        public: build_suite(
            SUITE,
            "F8/kontrakt",
            SuiteStatus::Proposed,
            &public_cases,
            Vec::new(),
        ),
        holdout: build_suite(
            SUITE,
            "kontrakt",
            SuiteStatus::Frozen,
            &cases(HOLDOUT_MARKER, Split::Holdout, 20, HOLDOUT_MARKER),
            Vec::new(),
        ),
        runner: Arc::new(QualityRunner::default()),
        policy,
    }
}

fn request(stage: GateStage, base: f64, cand: f64, repeats: u32) -> GateRequest {
    let variant = |id: &str, q: f64| Variant {
        id: id.into(),
        patch: BTreeMap::from([("quality".to_owned(), json!(q))]),
    };
    GateRequest {
        suite: SuiteId::new(SUITE).unwrap_or_else(|e| panic!("{e}")),
        stage,
        baseline: variant("baseline", base),
        candidate: variant("kandydat", cand),
        repeats,
        primary_metric: None,
    }
}

/// Uruchamia testy bramki; `factory` buduje bramkę ze scenariusza.
pub async fn run_gate_suite<F, Fut>(factory: F)
where
    F: Fn(GateScenario) -> Fut,
    Fut: Future<Output = Arc<dyn EvalGate>>,
{
    // Holdout: wynik zbiorczy, bez identyfikatorów, treści i nazw klas.
    let sc = standard_scenario(GatePolicy::default());
    let runner = Arc::clone(&sc.runner);
    let gate = factory(sc).await;
    let verdict = gate
        .evaluate(request(GateStage::Holdout, 0.5, 0.95, 5))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(verdict.passed(), "{verdict:?}");
    assert_eq!((verdict.n_cases, verdict.repeats), (20, 5));
    let text = serde_json::to_string(&verdict).unwrap_or_default();
    assert!(
        !text.contains(HOLDOUT_MARKER),
        "werdykt ujawnia holdout: {text}"
    );
    assert!(verdict.candidate.per_class.is_empty());
    assert_eq!(runner.seen().len(), 2 * 20 * 5);
    // Regresja odrzucona; deterministycznie.
    let worse = gate
        .evaluate(request(GateStage::Holdout, 0.5, 0.1, 5))
        .await;
    let worse = worse.unwrap_or_else(|e| panic!("{e}"));
    assert!(!worse.passed());
    let again = gate
        .evaluate(request(GateStage::Holdout, 0.5, 0.1, 5))
        .await;
    assert_eq!(again.unwrap_or_else(|e| panic!("{e}")), worse);
    // N < 5 — odmowa przed uruchomieniem czegokolwiek.
    let before = runner.seen().len();
    let few = gate
        .evaluate(request(GateStage::Holdout, 0.5, 0.9, 4))
        .await;
    assert!(matches!(few, Err(EvalError::TooFewRepeats { min: 5, .. })));
    assert_eq!(runner.seen().len(), before);
    assert!(gate.policy().validate().is_ok());

    // Piaskownica widzi tylko podział `test` zestawu publicznego.
    let sc = standard_scenario(GatePolicy::default());
    let runner = Arc::clone(&sc.runner);
    let gate = factory(sc).await;
    let v = gate
        .evaluate(request(GateStage::Sandbox, 0.5, 0.9, 5))
        .await;
    assert!(v.unwrap_or_else(|e| panic!("{e}")).passed());
    let seen: BTreeSet<String> = runner.seen().into_iter().collect();
    assert_eq!(seen.len(), 12);
    assert!(seen.iter().all(|id| id.starts_with("test-")), "{seen:?}");

    // Budżet zapytań do holdoutu.
    let policy = GatePolicy {
        max_holdout_queries: 2,
        ..GatePolicy::default()
    };
    let gate = factory(standard_scenario(policy)).await;
    for _ in 0..2 {
        let ok = gate
            .evaluate(request(GateStage::Holdout, 0.5, 0.6, 5))
            .await;
        assert!(ok.is_ok());
    }
    let over = gate
        .evaluate(request(GateStage::Holdout, 0.5, 0.6, 5))
        .await;
    assert_eq!(over, Err(EvalError::HoldoutBudgetExhausted(2)));

    // Błąd uruchomienia kandydata = odrzucenie (zachowawczo).
    let gate = factory(standard_scenario(GatePolicy::default())).await;
    let mut req = request(GateStage::Holdout, 0.5, 0.9, 5);
    req.candidate
        .patch
        .insert("fail_runner".into(), json!(true));
    let failed = gate.evaluate(req).await.unwrap_or_else(|e| panic!("{e}"));
    assert!(!failed.passed());

    // Zmieniony plik holdoutu blokuje bramkę.
    let sc = standard_scenario(GatePolicy::default());
    let path = sc.holdout.manifest.cases[0].path.clone();
    let sc = GateScenario {
        holdout: sc.holdout.tampered(&path, b"{}\n"),
        ..sc
    };
    let gate = factory(sc).await;
    let tampered = gate
        .evaluate(request(GateStage::Holdout, 0.5, 0.9, 5))
        .await;
    assert!(
        matches!(tampered, Err(EvalError::IntegrityViolation { .. })),
        "{tampered:?}"
    );
}

/// Uruchamia testy katalogu; `factory` buduje katalog z zestawów (korzeń `evals/`).
pub fn run_catalog_suite<F>(factory: F)
where
    F: Fn(Vec<SuiteFixture>) -> Arc<dyn SuiteCatalog>,
{
    let mut all = cases("d", Split::Dev, 3, "jawne");
    all.extend(cases("t", Split::Test, 4, "jawne"));
    let proposed = build_suite(
        "propozycja",
        "F8/p",
        SuiteStatus::Proposed,
        &all,
        Vec::new(),
    );
    let frozen = build_suite("zamrozony", "F8/z", SuiteStatus::Frozen, &all, Vec::new());
    let mut bad = build_suite("zly", "F8/zly", SuiteStatus::Proposed, &all, Vec::new());
    bad.manifest.cases[0].split = Some(Split::Holdout);

    let catalog = factory(vec![proposed.clone(), frozen.clone(), bad]);
    let listed: Vec<String> = catalog
        .suites()
        .iter()
        .map(|s| s.suite.to_string())
        .collect();
    assert_eq!(listed, ["propozycja", "zamrozony"]);
    let info = &catalog.suites()[0];
    assert_eq!(info.digest, proposed.manifest.digest());
    assert_eq!(info.case_counts.get(&Split::Dev), Some(&3));
    assert_eq!(info.case_counts.get(&Split::Test), Some(&4));
    assert!(!info.case_counts.contains_key(&Split::Holdout));

    let id = |s: &str| SuiteId::new(s).unwrap_or_else(|e| panic!("{e}"));
    let dev = catalog
        .cases(&id("propozycja"), Split::Dev)
        .unwrap_or_default();
    assert_eq!(dev.len(), 3);
    assert!(dev.iter().all(|c| c.split == Split::Dev));
    assert_eq!(
        catalog.cases(&id("propozycja"), Split::Holdout),
        Err(EvalError::HoldoutSealed)
    );
    assert!(catalog.cases(&id("zly"), Split::Dev).is_err());
    assert!(matches!(
        catalog.cases(&id("brak"), Split::Dev),
        Err(EvalError::UnknownSuite(_))
    ));
    assert!(
        catalog
            .verify(&id("zamrozony"))
            .is_ok_and(|r| r.is_intact())
    );

    // Zmiana pliku: zamrożony → błąd, propozycja → tylko raport rozjazdu.
    let path_f = frozen.manifest.cases[0].path.clone();
    let path_p = proposed.manifest.cases[0].path.clone();
    let catalog = factory(vec![
        proposed.tampered(&path_p, b"\n"),
        frozen.tampered(&path_f, b"\n"),
    ]);
    let r = catalog
        .verify(&id("zamrozony"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(!r.is_intact());
    assert!(matches!(
        catalog.cases(&id("zamrozony"), Split::Test),
        Err(EvalError::IntegrityViolation { .. })
    ));
    assert!(
        !catalog
            .verify(&id("propozycja"))
            .is_ok_and(|r| r.is_intact())
    );
    assert!(catalog.cases(&id("propozycja"), Split::Test).is_ok());
}
