//! Porównanie dwóch wariantów na tych samych przypadkach (bootstrap sparowany).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::aggregate::per_case_means;
use crate::case::CaseOutcome;
use crate::stats::{BootstrapConfig, Interval, bootstrap_mean, bootstrap_paired_delta};

/// Kierunek metryki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Więcej = lepiej (np. `pass_rate`).
    HigherIsBetter,
    /// Mniej = lepiej (np. `wer`, `latency_ms`).
    LowerIsBetter,
}

/// Werdykt porównania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompareVerdict {
    /// Przedział poprawy powyżej zera.
    Better,
    /// Przedział poprawy poniżej zera.
    Worse,
    /// Przedział obejmuje zero.
    NoSignificantDifference,
}

/// Porównanie dwóch wariantów na tych samych przypadkach.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VariantComparison {
    /// Metryka.
    pub metric: String,
    /// Kierunek.
    pub direction: Direction,
    /// Liczba sparowanych przypadków.
    pub n_cases: usize,
    /// Wariant bazowy.
    pub baseline: Interval,
    /// Kandydat.
    pub candidate: Interval,
    /// Poprawa (dodatnia = lepiej, niezależnie od kierunku metryki).
    pub improvement: Interval,
    /// Werdykt.
    pub verdict: CompareVerdict,
}

/// Porównanie sparowane po przypadkach obecnych w obu przebiegach.
pub fn compare(
    baseline: &[CaseOutcome],
    candidate: &[CaseOutcome],
    metric: &str,
    direction: Direction,
    cfg: &BootstrapConfig,
) -> Option<VariantComparison> {
    let b = per_case_means(baseline);
    let c = per_case_means(candidate);
    let (mut bs, mut cs) = (Vec::new(), Vec::new());
    for (id, row) in &b {
        if let (Some(bv), Some(cv)) = (
            row.metrics.get(metric),
            c.get(id).and_then(|r| r.metrics.get(metric)),
        ) {
            bs.push(*bv);
            cs.push(*cv);
        }
    }
    let sign = match direction {
        Direction::HigherIsBetter => 1.0,
        Direction::LowerIsBetter => -1.0,
    };
    let signed = |xs: &[f64]| xs.iter().map(|x| x * sign).collect::<Vec<_>>();
    let raw = bootstrap_paired_delta(&signed(&bs), &signed(&cs), cfg)?;
    let verdict = if raw.lo > 0.0 {
        CompareVerdict::Better
    } else if raw.hi < 0.0 {
        CompareVerdict::Worse
    } else {
        CompareVerdict::NoSignificantDifference
    };
    Some(VariantComparison {
        metric: metric.to_owned(),
        direction,
        n_cases: bs.len(),
        baseline: bootstrap_mean(&bs, cfg)?,
        candidate: bootstrap_mean(&cs, cfg)?,
        improvement: raw,
        verdict,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::aggregate::PASS_RATE;

    #[test]
    fn comparison_detects_direction() {
        let ids: Vec<String> = (0..30).map(|i| format!("c{i}")).collect();
        let mk = |ok: bool| -> Vec<CaseOutcome> {
            ids.iter()
                .map(|id| CaseOutcome {
                    case_id: id.clone(),
                    class: None,
                    repeat: 0,
                    passed: ok,
                    metrics: BTreeMap::from([("lat".to_owned(), if ok { 5.0 } else { 9.0 })]),
                })
                .collect()
        };
        let cfg = BootstrapConfig::default();
        let up = compare(
            &mk(false),
            &mk(true),
            PASS_RATE,
            Direction::HigherIsBetter,
            &cfg,
        )
        .unwrap();
        assert_eq!(up.verdict, CompareVerdict::Better);
        assert_eq!(up.n_cases, 30);
        let lat = compare(&mk(false), &mk(true), "lat", Direction::LowerIsBetter, &cfg).unwrap();
        assert_eq!(lat.verdict, CompareVerdict::Better);
        let same = compare(
            &mk(true),
            &mk(true),
            PASS_RATE,
            Direction::HigherIsBetter,
            &cfg,
        )
        .unwrap();
        assert_eq!(same.verdict, CompareVerdict::NoSignificantDifference);
        assert!(compare(&[], &[], PASS_RATE, Direction::HigherIsBetter, &cfg).is_none());
    }
}
