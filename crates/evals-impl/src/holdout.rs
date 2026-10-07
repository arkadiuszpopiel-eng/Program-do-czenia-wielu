//! Bramka ewaluacyjna z ukrytym holdoutem (PLAN §12.4, ACCEPTANCE F8-03). Holdout leży w osobnym
//! katalogu poza gitem (np. `%LOCALAPPDATA%\Alfa\evals\holdout`); bramka czyta go przy każdej
//! ocenie, ściśle sprawdza hashe i zwraca tylko wynik zbiorczy. Brak API zwracającego przypadki.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::{EventBus, Level};
use evals_contract::{
    CandidateRunner, Clock, EVENT_GATE_DECIDED, EVENT_INTEGRITY_FAILED, EvalCase, EvalError,
    EvalGate, GatePolicy, GateRequest, GateStage, GateVerdict, QueryBudget, Split, SuiteCatalog,
    SuiteId, SuiteManifest, evals_event, evaluate_cases, parse_cases, verify_files,
};
use serde_json::json;

use crate::dir::{discover, load_manifest, safe_read};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Bramka: piaskownica (podział `test` katalogu publicznego) i holdout z katalogu Jądra.
pub struct HoldoutGate {
    public: Arc<dyn SuiteCatalog>,
    root: PathBuf,
    holdout: BTreeMap<SuiteId, SuiteManifest>,
    runner: Arc<dyn CandidateRunner>,
    policy: GatePolicy,
    clock: Arc<dyn Clock>,
    budget: Mutex<QueryBudget>,
    bus: Option<Arc<dyn EventBus>>,
}

impl HoldoutGate {
    /// Otwiera holdout z `holdout_root` (manifesty `*.suite.json`, wyłącznie źródła `holdout`).
    /// Polityka słabsza niż plan albo niepoprawny manifest holdoutu → błąd (bramka nie startuje).
    pub fn open(
        holdout_root: impl AsRef<Path>,
        public: Arc<dyn SuiteCatalog>,
        runner: Arc<dyn CandidateRunner>,
        policy: GatePolicy,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, EvalError> {
        policy.validate()?;
        let root = fs::canonicalize(holdout_root.as_ref())
            .map_err(|e| EvalError::Io(format!("{}: {e}", holdout_root.as_ref().display())))?;
        let mut holdout = BTreeMap::new();
        for path in discover(&root, false) {
            let manifest = load_manifest(&root, &path).map_err(EvalError::InvalidManifest)?;
            manifest.validate_holdout()?;
            holdout.insert(manifest.suite.clone(), manifest);
        }
        Ok(Self {
            public,
            root,
            holdout,
            runner,
            policy,
            clock,
            budget: Mutex::new(QueryBudget::default()),
            bus: None,
        })
    }

    /// Magistrala zdarzeń (builder).
    #[must_use]
    pub fn with_bus(mut self, bus: Arc<dyn EventBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Identyfikatory zestawów holdout (bez przypadków).
    pub fn holdout_suites(&self) -> Vec<SuiteId> {
        self.holdout.keys().cloned().collect()
    }

    async fn emit(&self, name: &str, level: Level, payload: serde_json::Value) {
        if let Some(bus) = &self.bus {
            // Zdarzenie diagnostyczne; błąd magistrali nie zmienia werdyktu.
            let _ = bus.publish(evals_event(name, level, payload)).await;
        }
    }

    async fn holdout_cases(
        &self,
        suite: &SuiteId,
    ) -> Result<(SuiteManifest, Vec<EvalCase>), EvalError> {
        let manifest = self
            .holdout
            .get(suite)
            .ok_or_else(|| EvalError::UnknownSuite(suite.to_string()))?;
        let report = verify_files(manifest, |p| safe_read(&self.root, p, &[]).ok().flatten());
        if let Err(e) = report.enforce(true) {
            let payload = json!({
                "suite": suite.as_str(),
                "stage": "holdout",
                "mismatched": report.mismatched.len(),
                "missing": report.missing.len(),
            });
            self.emit(EVENT_INTEGRITY_FAILED, Level::Error, payload)
                .await;
            return Err(e);
        }
        let mut cases = Vec::new();
        for source in &manifest.cases {
            let bytes = safe_read(&self.root, &source.path, &[])?
                .ok_or_else(|| EvalError::Io("brak pliku holdoutu".into()))?;
            cases.extend(
                parse_cases(source, &bytes)?
                    .into_iter()
                    .filter(|c| c.split == Split::Holdout),
            );
        }
        Ok((manifest.clone(), cases))
    }
}

#[async_trait]
impl EvalGate for HoldoutGate {
    async fn evaluate(&self, request: GateRequest) -> Result<GateVerdict, EvalError> {
        let (manifest, cases) = match request.stage {
            GateStage::Sandbox => (
                self.public.manifest(&request.suite)?,
                self.public.cases(&request.suite, Split::Test)?,
            ),
            GateStage::Holdout => {
                let (manifest, cases) = self.holdout_cases(&request.suite).await?;
                request.check(&self.policy, cases.len())?;
                lock(&self.budget).try_acquire(self.clock.now_ms(), &self.policy)?;
                (manifest, cases)
            }
        };
        let verdict = evaluate_cases(
            &manifest,
            &cases,
            &request,
            &self.policy,
            self.runner.as_ref(),
        )
        .await?;
        let level = if verdict.passed() {
            Level::Info
        } else {
            Level::Warn
        };
        let payload = serde_json::to_value(&verdict).unwrap_or_default();
        self.emit(EVENT_GATE_DECIDED, level, payload).await;
        Ok(verdict)
    }

    fn policy(&self) -> GatePolicy {
        self.policy
    }
}
