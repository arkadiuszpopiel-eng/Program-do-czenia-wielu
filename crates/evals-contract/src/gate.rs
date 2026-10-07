//! Bramka ewaluacyjna (PLAN §12.4): porównanie wariantów przed/po na podziale `test`
//! (piaskownica) albo na ukrytym holdoucie — zawsze wynik zbiorczy, N ≥ 5 powtórzeń.

use std::collections::BTreeMap;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::aggregate::{Aggregate, PASS_RATE, ThresholdResult};
use crate::case::{CaseOutcome, EvalCase};
use crate::error::EvalError;
use crate::manifest::SuiteId;
use crate::stats::{BootstrapConfig, Interval};

/// Minimalna liczba powtórzeń wymagana planem (PLAN §12.4, ACCEPTANCE F8-03).
pub const MIN_GATE_REPEATS: u32 = 5;

/// Wariant systemu: identyfikator i łatka konfiguracji (klucz → wartość), nieprzezroczysta
/// dla `evals` — interpretuje ją [`CandidateRunner`] Jądra w piaskownicy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Variant {
    /// Identyfikator (np. `baseline`, `prop-17`).
    pub id: String,
    /// Łatka konfiguracji.
    #[serde(default)]
    pub patch: BTreeMap<String, Value>,
}

impl Variant {
    /// Wariant bez zmian.
    pub fn baseline() -> Self {
        Self {
            id: "baseline".into(),
            patch: BTreeMap::new(),
        }
    }
}

/// Etap bramki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateStage {
    /// Piaskownica: podział `test` zestawu publicznego.
    Sandbox,
    /// Ukryty holdout (poza gitem, poza zasięgiem Ulepszacza).
    Holdout,
}

/// Żądanie oceny.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GateRequest {
    /// Zestaw.
    pub suite: SuiteId,
    /// Etap.
    pub stage: GateStage,
    /// Wariant bazowy (bieżąca konfiguracja).
    pub baseline: Variant,
    /// Kandydat.
    pub candidate: Variant,
    /// Powtórzenia (≥ polityka).
    pub repeats: u32,
    /// Metryka główna (domyślnie `pass_rate`, więcej = lepiej).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_metric: Option<String>,
}

/// Polityka bramki — własność Jądra; Ulepszacz tylko ją odczytuje.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GatePolicy {
    /// Minimalne powtórzenia (nie mniej niż [`MIN_GATE_REPEATS`]).
    pub min_repeats: u32,
    /// Minimalna liczba przypadków.
    pub min_cases: usize,
    /// Dopuszczalny spadek metryki głównej na dolnej granicy przedziału poprawy.
    pub max_regression: f64,
    /// Wymagana średnia poprawa.
    pub min_improvement: f64,
    /// Limit ocen na holdoucie w oknie.
    pub max_holdout_queries: u32,
    /// Okno limitu (ms).
    pub budget_window_ms: u64,
    /// Zaokrąglenie liczb w werdykcie (mniej informacji o pojedynczych przypadkach).
    pub report_decimals: u32,
    /// Bootstrap.
    pub bootstrap: BootstrapConfig,
}

impl Default for GatePolicy {
    fn default() -> Self {
        Self {
            min_repeats: MIN_GATE_REPEATS,
            min_cases: 10,
            max_regression: 0.0,
            min_improvement: 0.0,
            max_holdout_queries: 20,
            budget_window_ms: 24 * 60 * 60 * 1000,
            report_decimals: 3,
            bootstrap: BootstrapConfig::default(),
        }
    }
}

impl GatePolicy {
    /// Polityka nie może być słabsza niż plan (N ≥ 5, regresja w [0, 1], limit zapytań > 0).
    pub fn validate(&self) -> Result<(), EvalError> {
        let bad = |m: &str| Err(EvalError::InvalidPolicy(m.to_owned()));
        if self.min_repeats < MIN_GATE_REPEATS {
            return bad("min_repeats < 5 (PLAN §12.4)");
        }
        if !(0.0..=1.0).contains(&self.max_regression) || !self.min_improvement.is_finite() {
            return bad("max_regression poza [0, 1] albo min_improvement nieskończone");
        }
        if self.min_cases == 0 || self.max_holdout_queries == 0 || self.budget_window_ms == 0 {
            return bad("min_cases, max_holdout_queries i budget_window_ms muszą być > 0");
        }
        Ok(())
    }
}

/// Podsumowanie wariantu — tylko liczby.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VariantSummary {
    /// Metryka → przedział.
    pub metrics: BTreeMap<String, Interval>,
    /// Klasa → odsetek zaliczonych.
    pub per_class: BTreeMap<String, Interval>,
}

impl VariantSummary {
    pub(crate) fn from_aggregate(agg: &Aggregate, decimals: u32) -> Self {
        Self {
            metrics: agg
                .metrics
                .iter()
                .map(|(k, v)| (k.clone(), v.rounded(decimals)))
                .collect(),
            per_class: agg
                .per_class
                .iter()
                .filter_map(|(k, c)| {
                    c.metrics
                        .get(PASS_RATE)
                        .map(|i| (k.clone(), i.rounded(decimals)))
                })
                .collect(),
        }
    }
}

/// Decyzja.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum GateDecision {
    /// Kandydat przechodzi.
    Pass,
    /// Kandydat odrzucony.
    Fail {
        /// Powody (po polsku, bez danych przypadków).
        reasons: Vec<String>,
    },
}

/// Werdykt — wynik zbiorczy, bez identyfikatorów i treści przypadków.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GateVerdict {
    /// Zestaw.
    pub suite: SuiteId,
    /// Etap.
    pub stage: GateStage,
    /// Hash manifestu zestawu w chwili oceny.
    pub suite_digest: String,
    /// Liczba przypadków.
    pub n_cases: usize,
    /// Powtórzenia.
    pub repeats: u32,
    /// Metryka główna.
    pub primary_metric: String,
    /// Wariant bazowy.
    pub baseline: VariantSummary,
    /// Kandydat.
    pub candidate: VariantSummary,
    /// Poprawa metryki głównej (kandydat − baza).
    pub improvement: Interval,
    /// Progi zestawu na kandydacie (bez nazw klas dla holdoutu).
    pub thresholds: Vec<ThresholdResult>,
    /// Decyzja.
    pub decision: GateDecision,
}

impl GateVerdict {
    /// Czy przeszedł.
    pub fn passed(&self) -> bool {
        self.decision == GateDecision::Pass
    }
}

/// Uruchamia system z wariantem w piaskownicy (dostarcza Jądro — nigdy Ulepszacz).
#[async_trait]
pub trait CandidateRunner: Send + Sync {
    /// Jedno powtórzenie przypadku.
    async fn run(
        &self,
        variant: &Variant,
        case: &EvalCase,
        repeat: u32,
    ) -> Result<CaseOutcome, String>;
}

/// Bramka ewaluacyjna (Jądro). Jedyna droga do holdoutu; zwraca wyłącznie wynik zbiorczy.
#[async_trait]
pub trait EvalGate: Send + Sync {
    /// Ocena kandydata względem bazy.
    async fn evaluate(&self, request: GateRequest) -> Result<GateVerdict, EvalError>;
    /// Polityka (tylko odczyt — brak metody zmiany).
    fn policy(&self) -> GatePolicy;
}

/// Walidacja żądania względem polityki (powtórzenia, liczba przypadków).
pub(crate) fn check_request(
    request: &GateRequest,
    policy: &GatePolicy,
    n_cases: usize,
) -> Result<(), EvalError> {
    if request.repeats < policy.min_repeats.max(MIN_GATE_REPEATS) {
        return Err(EvalError::TooFewRepeats {
            got: request.repeats,
            min: policy.min_repeats.max(MIN_GATE_REPEATS),
        });
    }
    if n_cases < policy.min_cases {
        return Err(EvalError::TooFewCases {
            got: n_cases,
            min: policy.min_cases,
        });
    }
    Ok(())
}

impl GateRequest {
    /// Sprawdza żądanie (powtórzenia ≥ polityka i ≥ 5, przypadków ≥ `min_cases`).
    pub fn check(&self, policy: &GatePolicy, n_cases: usize) -> Result<(), EvalError> {
        check_request(self, policy, n_cases)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_cannot_be_weaker_than_plan() {
        GatePolicy::default().validate().unwrap();
        for p in [
            GatePolicy {
                min_repeats: 4,
                ..GatePolicy::default()
            },
            GatePolicy {
                max_regression: 2.0,
                ..GatePolicy::default()
            },
            GatePolicy {
                max_holdout_queries: 0,
                ..GatePolicy::default()
            },
        ] {
            assert!(matches!(p.validate(), Err(EvalError::InvalidPolicy(_))));
        }
    }

    #[test]
    fn request_check_enforces_minimums() {
        let req = GateRequest {
            suite: SuiteId::new("s").unwrap(),
            stage: GateStage::Holdout,
            baseline: Variant::baseline(),
            candidate: Variant::baseline(),
            repeats: 3,
            primary_metric: None,
        };
        let weak = GatePolicy {
            min_repeats: 1,
            ..GatePolicy::default()
        };
        assert!(matches!(
            req.check(&weak, 50),
            Err(EvalError::TooFewRepeats { min: 5, .. })
        ));
        let ok = GateRequest { repeats: 5, ..req };
        assert!(matches!(
            ok.check(&weak, 3),
            Err(EvalError::TooFewCases { .. })
        ));
        ok.check(&weak, 10).unwrap();
    }
}
