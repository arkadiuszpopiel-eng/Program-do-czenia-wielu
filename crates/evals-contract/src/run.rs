//! Rdzeń bramki wspólny dla `-impl` i `-fake`: przebieg wariantów, decyzja, budżet holdoutu.

use std::collections::VecDeque;

use crate::aggregate::{PASS_RATE, RUNNER_ERROR, ThresholdStatus, aggregate, check_thresholds};
use crate::case::{CaseOutcome, EvalCase};
use crate::compare::{Direction, compare};
use crate::error::EvalError;
use crate::gate::{
    CandidateRunner, GateDecision, GatePolicy, GateRequest, GateStage, GateVerdict, Variant,
    VariantSummary,
};
use crate::manifest::SuiteManifest;
use crate::stats::Interval;

/// Przebieg wariantu: każdy przypadek × powtórzenia; błąd uruchomienia = niezaliczone
/// powtórzenie z metryką [`RUNNER_ERROR`] (zachowawczo).
pub async fn run_variant(
    runner: &dyn CandidateRunner,
    variant: &Variant,
    cases: &[EvalCase],
    repeats: u32,
) -> Vec<CaseOutcome> {
    let mut out = Vec::with_capacity(cases.len() * usize::try_from(repeats).unwrap_or(0));
    for case in cases {
        for repeat in 0..repeats {
            let outcome = match runner.run(variant, case, repeat).await {
                Ok(mut o) => {
                    o.case_id.clone_from(&case.id);
                    o.class.clone_from(&case.class);
                    o.repeat = repeat;
                    o.with_metric(RUNNER_ERROR, 0.0)
                }
                Err(_) => CaseOutcome::new(case, repeat, false).with_metric(RUNNER_ERROR, 1.0),
            };
            out.push(outcome);
        }
    }
    out
}

/// Decyzja bramki z wyników obu wariantów (wspólna dla `-impl` i `-fake`).
pub fn decide(
    manifest: &SuiteManifest,
    request: &GateRequest,
    policy: &GatePolicy,
    baseline: &[CaseOutcome],
    candidate: &[CaseOutcome],
) -> GateVerdict {
    let cfg = &policy.bootstrap;
    let metric = request
        .primary_metric
        .clone()
        .unwrap_or_else(|| PASS_RATE.to_owned());
    let base = aggregate(baseline, cfg);
    let cand = aggregate(candidate, cfg);
    let mut reasons = Vec::new();
    let improvement = match compare(baseline, candidate, &metric, Direction::HigherIsBetter, cfg) {
        Some(c) => c.improvement,
        None => {
            reasons.push(format!("brak metryki `{metric}` w wynikach"));
            Interval::point(0.0)
        }
    };
    if improvement.lo < -policy.max_regression {
        reasons.push(format!(
            "regresja `{metric}`: dolna granica poprawy {:.3} < −{:.3}",
            improvement.lo, policy.max_regression
        ));
    }
    if improvement.mean < policy.min_improvement {
        reasons.push(format!(
            "za mała poprawa `{metric}`: {:.3} < {:.3}",
            improvement.mean, policy.min_improvement
        ));
    }
    let mut thresholds = check_thresholds(&cand, &manifest.thresholds);
    if thresholds
        .iter()
        .any(|t| t.status == ThresholdStatus::Failed)
    {
        reasons.push("kandydat nie spełnia progów zestawu".into());
    }
    let hide_classes = request.stage == GateStage::Holdout;
    for t in &mut thresholds {
        t.observed = t
            .observed
            .map(|v| Interval::point(v).rounded(policy.report_decimals).mean);
        if hide_classes {
            t.class = None;
        }
    }
    let mut baseline_summary = VariantSummary::from_aggregate(&base, policy.report_decimals);
    let mut candidate_summary = VariantSummary::from_aggregate(&cand, policy.report_decimals);
    if hide_classes {
        baseline_summary.per_class.clear();
        candidate_summary.per_class.clear();
    }
    GateVerdict {
        suite: manifest.suite.clone(),
        stage: request.stage,
        suite_digest: manifest.digest(),
        n_cases: cand.n_cases,
        repeats: request.repeats,
        primary_metric: metric,
        baseline: baseline_summary,
        candidate: candidate_summary,
        improvement: improvement.rounded(policy.report_decimals),
        thresholds,
        decision: if reasons.is_empty() {
            GateDecision::Pass
        } else {
            GateDecision::Fail { reasons }
        },
    }
}

/// Ocena na gotowych przypadkach (wspólna dla `-impl` i `-fake`): walidacja polityki
/// i żądania, przebieg obu wariantów, decyzja. Budżet holdoutu rezerwuje wywołujący.
pub async fn evaluate_cases(
    manifest: &SuiteManifest,
    cases: &[EvalCase],
    request: &GateRequest,
    policy: &GatePolicy,
    runner: &dyn CandidateRunner,
) -> Result<GateVerdict, EvalError> {
    policy.validate()?;
    request.check(policy, cases.len())?;
    let baseline = run_variant(runner, &request.baseline, cases, request.repeats).await;
    let candidate = run_variant(runner, &request.candidate, cases, request.repeats).await;
    Ok(decide(manifest, request, policy, &baseline, &candidate))
}

/// Budżet zapytań do holdoutu w oknie przesuwnym.
#[derive(Debug, Clone, Default)]
pub struct QueryBudget {
    stamps: VecDeque<u64>,
}

impl QueryBudget {
    /// Rezerwuje zapytanie o `now_ms` albo zwraca [`EvalError::HoldoutBudgetExhausted`].
    pub fn try_acquire(&mut self, now_ms: u64, policy: &GatePolicy) -> Result<(), EvalError> {
        while self
            .stamps
            .front()
            .is_some_and(|t| now_ms.saturating_sub(*t) >= policy.budget_window_ms)
        {
            self.stamps.pop_front();
        }
        let used = u32::try_from(self.stamps.len()).unwrap_or(u32::MAX);
        if used >= policy.max_holdout_queries {
            return Err(EvalError::HoldoutBudgetExhausted(
                policy.max_holdout_queries,
            ));
        }
        self.stamps.push_back(now_ms);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_slides() {
        let policy = GatePolicy {
            max_holdout_queries: 2,
            budget_window_ms: 100,
            ..GatePolicy::default()
        };
        let mut b = QueryBudget::default();
        b.try_acquire(0, &policy).unwrap();
        b.try_acquire(10, &policy).unwrap();
        assert_eq!(
            b.try_acquire(50, &policy),
            Err(EvalError::HoldoutBudgetExhausted(2))
        );
        b.try_acquire(100, &policy).unwrap();
    }
}
