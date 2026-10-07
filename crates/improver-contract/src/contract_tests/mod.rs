//! Współdzielone testy kontraktowe Ulepszacza (feature `contract-tests`), uruchamiane na `-impl`
//! i `-fake`. Magazyn konfiguracji (np. `core-config-fake`) dostarcza wywołujący — kontrakt nie
//! zależy od atrap innych modułów.

mod attacks;
mod pipeline;

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use core_bus_contract::Event;
use core_config_contract::{ConfigStore, Origin};
use evals_contract::{
    EvalError, EvalGate, GateDecision, GatePolicy, GateRequest, GateStage, GateVerdict, Interval,
    VariantSummary,
};
use serde_json::Value;

use crate::{
    ApprovalVerifier, CandidateSet, Improver, ImproverPolicy, MetricsSnapshot, Proposal, Proposer,
    Ring, UserApproval,
};

pub use attacks::{
    Attack, AttackReport, AttackSet, AttackVia, R0Case, R0Report, R0Set, run_attacks, run_r0_cases,
};

/// Bramka skryptowana: przechodzi, chyba że łatka kandydata dotyka klucza z listy porażek.
#[derive(Debug, Default)]
pub struct ScriptedGate {
    fail_sandbox: Mutex<BTreeSet<String>>,
    fail_holdout: Mutex<BTreeSet<String>>,
    requests: Mutex<Vec<GateRequest>>,
}

impl ScriptedGate {
    /// Klucz, którego zmiana oblewa piaskownicę.
    pub fn fail_sandbox_on(&self, key: &str) {
        if let Ok(mut g) = self.fail_sandbox.lock() {
            g.insert(key.to_owned());
        }
    }

    /// Klucz, którego zmiana oblewa holdout.
    pub fn fail_holdout_on(&self, key: &str) {
        if let Ok(mut g) = self.fail_holdout.lock() {
            g.insert(key.to_owned());
        }
    }

    /// Przyjęte żądania.
    pub fn requests(&self) -> Vec<GateRequest> {
        self.requests.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl EvalGate for ScriptedGate {
    async fn evaluate(&self, request: GateRequest) -> Result<GateVerdict, EvalError> {
        if let Ok(mut g) = self.requests.lock() {
            g.push(request.clone());
        }
        request.check(&self.policy(), 10)?;
        let fails = match request.stage {
            GateStage::Sandbox => &self.fail_sandbox,
            GateStage::Holdout => &self.fail_holdout,
        };
        let fail = fails
            .lock()
            .map(|f| request.candidate.patch.keys().any(|k| f.contains(k)))
            .unwrap_or(true);
        let summary = VariantSummary {
            metrics: BTreeMap::new(),
            per_class: BTreeMap::new(),
        };
        Ok(GateVerdict {
            suite: request.suite.clone(),
            stage: request.stage,
            suite_digest: "0".repeat(64),
            n_cases: 10,
            repeats: request.repeats,
            primary_metric: "pass_rate".into(),
            baseline: summary.clone(),
            candidate: summary,
            improvement: Interval::point(if fail { -0.2 } else { 0.1 }),
            thresholds: Vec::new(),
            decision: if fail {
                GateDecision::Fail {
                    reasons: vec!["regresja".into()],
                }
            } else {
                GateDecision::Pass
            },
        })
    }

    fn policy(&self) -> GatePolicy {
        GatePolicy::default()
    }
}

/// Weryfikator testowy: R0 — wystarczy zgodny diff; R1/R2 — podpis `tpm:<digest>`.
#[derive(Debug, Default)]
pub struct TestVerifier;

/// Podpis, który akceptuje [`TestVerifier`].
pub fn signature_for(digest: &str) -> String {
    format!("tpm:{digest}")
}

impl ApprovalVerifier for TestVerifier {
    fn verify(&self, approval: &UserApproval, proposal: &Proposal) -> bool {
        approval.proposal == proposal.id
            && approval.digest == proposal.digest
            && (proposal.ring == Ring::R0
                || approval.signature.as_deref() == Some(signature_for(&proposal.digest).as_str()))
    }
}

/// Źródło skryptowane (np. „model” zwracający złośliwe zmiany).
#[derive(Debug, Default)]
pub struct ScriptedProposer {
    sets: Mutex<Vec<CandidateSet>>,
}

impl ScriptedProposer {
    /// Źródło zwracające te zestawy przy każdej obserwacji.
    pub fn new(sets: Vec<CandidateSet>) -> Self {
        Self {
            sets: Mutex::new(sets),
        }
    }
}

#[async_trait]
impl Proposer for ScriptedProposer {
    fn name(&self) -> String {
        "skryptowany".into()
    }

    async fn propose(&self, _: &MetricsSnapshot) -> Result<Vec<CandidateSet>, String> {
        Ok(self.sets.lock().map(|g| g.clone()).unwrap_or_default())
    }
}

/// Parametry budowy Ulepszacza w teście.
pub struct Setup {
    /// Bramka.
    pub gate: Arc<ScriptedGate>,
    /// Weryfikator zatwierdzeń.
    pub verifier: Arc<dyn ApprovalVerifier>,
    /// Polityka.
    pub policy: ImproverPolicy,
    /// Wartości początkowe konfiguracji (warstwa wspólna, `Origin::User`).
    pub initial: Vec<(String, Value)>,
    /// Dodatkowe źródła propozycji.
    pub proposers: Vec<Arc<dyn Proposer>>,
}

impl Setup {
    /// Domyślne: bramka przepuszczająca, weryfikator testowy, polityka domyślna.
    pub fn new(initial: Vec<(String, Value)>) -> Self {
        Self {
            gate: Arc::new(ScriptedGate::default()),
            verifier: Arc::new(TestVerifier),
            policy: ImproverPolicy::default(),
            initial,
            proposers: Vec::new(),
        }
    }
}

/// Zapis zarejestrowany w historii magazynu konfiguracji.
pub type WriteLog = Arc<dyn Fn() -> Vec<(String, Origin)> + Send + Sync>;

/// Ulepszacz zbudowany przez wywołującego z dostępem do magazynu i jego historii.
pub struct Harness {
    /// Ulepszacz.
    pub improver: Arc<dyn Improver>,
    /// Ten sam magazyn, do którego pisze Ulepszacz.
    pub config: Arc<dyn ConfigStore>,
    /// Udane zapisy (klucz, inicjator) od utworzenia.
    pub writes: WriteLog,
    /// Wyemitowane zdarzenia.
    pub events: Arc<dyn Fn() -> Vec<Event> + Send + Sync>,
    /// Przesunięcie wirtualnego zegara (ms).
    pub advance: Arc<dyn Fn(u64) + Send + Sync>,
}

impl Harness {
    /// Zapisy wykonane przez Ulepszacza.
    pub fn improver_writes(&self) -> Vec<String> {
        (self.writes)()
            .into_iter()
            .filter(|(_, o)| *o == Origin::Improver)
            .map(|(k, _)| k)
            .collect()
    }
}

/// Uruchamia testy potoku na implementacji zbudowanej przez `factory`.
pub async fn run_all<F, Fut>(factory: F)
where
    F: Fn(Setup) -> Fut,
    Fut: Future<Output = Harness>,
{
    pipeline::r0_auto_deploy_and_regression_rollback(&factory).await;
    pipeline::widening_and_r1_need_valid_approval(&factory).await;
    pipeline::gate_failures_block_deploy(&factory).await;
    pipeline::conditions_conflicts_and_atomic_sets(&factory).await;
    pipeline::untrusted_proposer_and_code_drafts(&factory).await;
}
