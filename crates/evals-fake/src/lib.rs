//! Atrapa harnessu ewaluacji (docs/modules/evals/SPEC.md „Fake”): katalog i bramka w pamięci.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use evals_contract::{
    CandidateRunner, Clock, EvalCase, EvalError, EvalGate, GatePolicy, GateRequest, GateStage,
    GateVerdict, IntegrityReport, ManualClock, QueryBudget, Split, SuiteCatalog, SuiteId,
    SuiteInfo, SuiteManifest, evaluate_cases, parse_cases, verify_files,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Zestaw w pamięci.
#[derive(Debug, Clone)]
struct Stored {
    manifest: SuiteManifest,
    files: BTreeMap<String, Vec<u8>>,
}

impl Stored {
    fn verify(&self) -> IntegrityReport {
        verify_files(&self.manifest, |p| self.files.get(p).cloned())
    }

    fn cases(&self, split: Split) -> Result<Vec<EvalCase>, EvalError> {
        let mut out = Vec::new();
        for source in &self.manifest.cases {
            let bytes = self
                .files
                .get(&source.path)
                .ok_or_else(|| EvalError::Io(format!("brak pliku `{}`", source.path)))?;
            out.extend(
                parse_cases(source, bytes)?
                    .into_iter()
                    .filter(|c| c.split == split),
            );
        }
        Ok(out)
    }
}

/// Katalog zestawów publicznych w pamięci.
#[derive(Debug, Default)]
pub struct FakeCatalog {
    suites: Mutex<BTreeMap<SuiteId, Stored>>,
}

impl FakeCatalog {
    /// Pusty katalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dodaje zestaw publiczny; manifest z holdoutem albo niepoprawny → błąd (nie trafia do katalogu).
    pub fn add_suite(
        &self,
        manifest: SuiteManifest,
        files: BTreeMap<String, Vec<u8>>,
    ) -> Result<(), EvalError> {
        manifest.validate_public()?;
        lock(&self.suites).insert(manifest.suite.clone(), Stored { manifest, files });
        Ok(())
    }

    /// Podmienia treść pliku (symulacja zmiany zestawu bez nowego manifestu).
    pub fn tamper(&self, path: &str, bytes: &[u8]) {
        for stored in lock(&self.suites).values_mut() {
            if let Some(slot) = stored.files.get_mut(path) {
                *slot = bytes.to_vec();
            }
        }
    }

    fn get(&self, suite: &SuiteId) -> Result<Stored, EvalError> {
        lock(&self.suites)
            .get(suite)
            .cloned()
            .ok_or_else(|| EvalError::UnknownSuite(suite.to_string()))
    }
}

impl SuiteCatalog for FakeCatalog {
    fn suites(&self) -> Vec<SuiteInfo> {
        lock(&self.suites)
            .values()
            .map(|s| SuiteInfo {
                suite: s.manifest.suite.clone(),
                wave: s.manifest.wave.clone(),
                version: s.manifest.version,
                status: s.manifest.status,
                digest: s.manifest.digest(),
                manifest_path: format!("{}.suite.json", s.manifest.suite),
                case_counts: [Split::Dev, Split::Test]
                    .into_iter()
                    .filter_map(|sp| {
                        let n = s.cases(sp).map(|c| c.len()).unwrap_or(0);
                        (n > 0).then_some((sp, n))
                    })
                    .collect(),
            })
            .collect()
    }

    fn manifest(&self, suite: &SuiteId) -> Result<SuiteManifest, EvalError> {
        Ok(self.get(suite)?.manifest)
    }

    fn verify(&self, suite: &SuiteId) -> Result<IntegrityReport, EvalError> {
        Ok(self.get(suite)?.verify())
    }

    fn cases(&self, suite: &SuiteId, split: Split) -> Result<Vec<EvalCase>, EvalError> {
        if split == Split::Holdout {
            return Err(EvalError::HoldoutSealed);
        }
        let stored = self.get(suite)?;
        stored.verify().enforce(false)?;
        stored.cases(split)
    }
}

/// Bramka z holdoutem w pamięci.
pub struct FakeGate {
    catalog: Arc<dyn SuiteCatalog>,
    holdout: BTreeMap<SuiteId, Stored>,
    runner: Arc<dyn CandidateRunner>,
    policy: GatePolicy,
    clock: Arc<dyn Clock>,
    budget: Mutex<QueryBudget>,
    requests: Mutex<Vec<GateRequest>>,
}

impl FakeGate {
    /// Bramka z katalogiem publicznym, runnerem i polityką; zegar wirtualny od 0.
    pub fn new(
        catalog: Arc<dyn SuiteCatalog>,
        runner: Arc<dyn CandidateRunner>,
        policy: GatePolicy,
    ) -> Self {
        Self {
            catalog,
            holdout: BTreeMap::new(),
            runner,
            policy,
            clock: Arc::new(ManualClock::new(0)),
            budget: Mutex::new(QueryBudget::default()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Zegar (builder).
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Dodaje zestaw holdout (builder); manifest musi mieć wyłącznie źródła `holdout`.
    pub fn with_holdout(
        mut self,
        manifest: SuiteManifest,
        files: BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, EvalError> {
        manifest.validate_holdout()?;
        self.holdout
            .insert(manifest.suite.clone(), Stored { manifest, files });
        Ok(self)
    }

    /// Żądania przyjęte do oceny (także odrzucone).
    pub fn requests(&self) -> Vec<GateRequest> {
        lock(&self.requests).clone()
    }
}

#[async_trait]
impl EvalGate for FakeGate {
    async fn evaluate(&self, request: GateRequest) -> Result<GateVerdict, EvalError> {
        lock(&self.requests).push(request.clone());
        self.policy.validate()?;
        let (manifest, cases) = match request.stage {
            GateStage::Sandbox => (
                self.catalog.manifest(&request.suite)?,
                self.catalog.cases(&request.suite, Split::Test)?,
            ),
            GateStage::Holdout => {
                let stored = self
                    .holdout
                    .get(&request.suite)
                    .ok_or_else(|| EvalError::UnknownSuite(request.suite.to_string()))?;
                stored.verify().enforce(true)?;
                let cases = stored.cases(Split::Holdout)?;
                request.check(&self.policy, cases.len())?;
                lock(&self.budget).try_acquire(self.clock.now_ms(), &self.policy)?;
                (stored.manifest.clone(), cases)
            }
        };
        evaluate_cases(
            &manifest,
            &cases,
            &request,
            &self.policy,
            self.runner.as_ref(),
        )
        .await
    }

    fn policy(&self) -> GatePolicy {
        self.policy
    }
}
