//! Agregacja wyników (jednostką próby jest przypadek — powtórzenia uśredniane najpierw),
//! sprawdzanie progów i porównanie wariantów.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::case::CaseOutcome;
use crate::manifest::{Comparison, Threshold, ThresholdRule};
use crate::stats::{BootstrapConfig, Interval, bootstrap_mean};

/// Nazwa metryki odsetka zaliczonych.
pub const PASS_RATE: &str = "pass_rate";
/// Metryka: odsetek powtórzeń zakończonych błędem uruchomienia (0/1 na powtórzenie).
pub const RUNNER_ERROR: &str = "runner_error";

/// Średnie metryk jednego przypadku po powtórzeniach.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseMeans {
    /// Klasa.
    pub class: Option<String>,
    /// Metryka → średnia po powtórzeniach (zawsze z [`PASS_RATE`]).
    pub metrics: BTreeMap<String, f64>,
}

/// Suma i liczność metryk jednego przypadku.
type Sums<'a> = (Option<&'a String>, BTreeMap<&'a str, (f64, u32)>);

/// Średnie per przypadek (klucz: identyfikator przypadku).
pub fn per_case_means(outcomes: &[CaseOutcome]) -> BTreeMap<String, CaseMeans> {
    let mut sums: BTreeMap<&str, Sums<'_>> = BTreeMap::new();
    for o in outcomes {
        let entry = sums
            .entry(o.case_id.as_str())
            .or_insert_with(|| (o.class.as_ref(), BTreeMap::new()));
        let pass = entry.1.entry(PASS_RATE).or_insert((0.0, 0));
        pass.0 += if o.passed { 1.0 } else { 0.0 };
        pass.1 += 1;
        for (name, value) in &o.metrics {
            let m = entry.1.entry(name.as_str()).or_insert((0.0, 0));
            m.0 += value;
            m.1 += 1;
        }
    }
    sums.into_iter()
        .map(|(id, (class, metrics))| {
            let metrics = metrics
                .into_iter()
                .map(|(k, (sum, n))| (k.to_owned(), sum / f64::from(n.max(1))))
                .collect();
            (
                id.to_owned(),
                CaseMeans {
                    class: class.cloned(),
                    metrics,
                },
            )
        })
        .collect()
}

/// Agregat klasy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ClassAggregate {
    /// Liczba przypadków.
    pub n_cases: usize,
    /// Metryka → przedział.
    pub metrics: BTreeMap<String, Interval>,
}

/// Agregat przebiegu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Aggregate {
    /// Liczba przypadków.
    pub n_cases: usize,
    /// Największa liczba powtórzeń przypadku.
    pub repeats: u32,
    /// Metryka → przedział (bootstrap po przypadkach).
    pub metrics: BTreeMap<String, Interval>,
    /// Klasa → agregat.
    pub per_class: BTreeMap<String, ClassAggregate>,
}

fn intervals(rows: &[&CaseMeans], cfg: &BootstrapConfig) -> BTreeMap<String, Interval> {
    let mut by_metric: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for row in rows {
        for (name, value) in &row.metrics {
            by_metric.entry(name.as_str()).or_default().push(*value);
        }
    }
    by_metric
        .into_iter()
        .filter_map(|(name, xs)| bootstrap_mean(&xs, cfg).map(|i| (name.to_owned(), i)))
        .collect()
}

/// Agregacja wyników przebiegu.
pub fn aggregate(outcomes: &[CaseOutcome], cfg: &BootstrapConfig) -> Aggregate {
    let means = per_case_means(outcomes);
    let rows: Vec<&CaseMeans> = means.values().collect();
    let mut classes: BTreeMap<&str, Vec<&CaseMeans>> = BTreeMap::new();
    for row in &rows {
        if let Some(class) = &row.class {
            classes.entry(class.as_str()).or_default().push(row);
        }
    }
    let repeats = {
        let mut per_case: BTreeMap<&str, u32> = BTreeMap::new();
        for o in outcomes {
            *per_case.entry(o.case_id.as_str()).or_default() += 1;
        }
        per_case.values().copied().max().unwrap_or(0)
    };
    Aggregate {
        n_cases: rows.len(),
        repeats,
        metrics: intervals(&rows, cfg),
        per_class: classes
            .into_iter()
            .map(|(class, rows)| {
                let agg = ClassAggregate {
                    n_cases: rows.len(),
                    metrics: intervals(&rows, cfg),
                };
                (class.to_owned(), agg)
            })
            .collect(),
    }
}

/// Wynik sprawdzenia progu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdStatus {
    /// Spełniony.
    Passed,
    /// Niespełniony (albo brak metryki).
    Failed,
    /// Próg opisowy — sprawdza test modułu albo człowiek.
    NotEvaluated,
}

/// Wynik progu (dla progu per klasa — jeden wpis na klasę).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ThresholdResult {
    /// ID kryterium.
    pub id: String,
    /// Klasa (próg per klasa).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// Wartość porównana z progiem (granica przedziału albo średnia).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<f64>,
    /// Próg.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Status.
    pub status: ThresholdStatus,
}

fn judge(
    interval: Option<&Interval>,
    op: Comparison,
    value: f64,
    point: bool,
) -> (Option<f64>, ThresholdStatus) {
    let Some(i) = interval else {
        return (None, ThresholdStatus::Failed);
    };
    let observed = match (point, op) {
        (true, _) => i.mean,
        (false, Comparison::Ge) => i.lo,
        (false, Comparison::Le) => i.hi,
    };
    let ok = match op {
        Comparison::Ge => observed >= value,
        Comparison::Le => observed <= value,
    };
    let status = if ok {
        ThresholdStatus::Passed
    } else {
        ThresholdStatus::Failed
    };
    (Some(observed), status)
}

/// Sprawdza progi na agregacie (dolna granica dla `≥`, górna dla `≤`, chyba że `point`).
pub fn check_thresholds(agg: &Aggregate, thresholds: &[Threshold]) -> Vec<ThresholdResult> {
    let mut out = Vec::new();
    for t in thresholds {
        match &t.rule {
            ThresholdRule::Text { .. } => out.push(ThresholdResult {
                id: t.id.clone(),
                class: None,
                observed: None,
                threshold: None,
                status: ThresholdStatus::NotEvaluated,
            }),
            ThresholdRule::Metric {
                metric,
                op,
                value,
                per_class,
                point,
            } if *per_class => {
                if agg.per_class.is_empty() {
                    out.push(ThresholdResult {
                        id: t.id.clone(),
                        class: None,
                        observed: None,
                        threshold: Some(*value),
                        status: ThresholdStatus::Failed,
                    });
                }
                for (class, c) in &agg.per_class {
                    let (observed, status) = judge(c.metrics.get(metric), *op, *value, *point);
                    out.push(ThresholdResult {
                        id: t.id.clone(),
                        class: Some(class.clone()),
                        observed,
                        threshold: Some(*value),
                        status,
                    });
                }
            }
            ThresholdRule::Metric {
                metric,
                op,
                value,
                point,
                ..
            } => {
                let (observed, status) = judge(agg.metrics.get(metric), *op, *value, *point);
                out.push(ThresholdResult {
                    id: t.id.clone(),
                    class: None,
                    observed,
                    threshold: Some(*value),
                    status,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::case::EvalCase;
    use crate::manifest::Split;

    fn outcomes(pass: &[(&str, &str, bool)]) -> Vec<CaseOutcome> {
        pass.iter()
            .enumerate()
            .map(|(i, (id, class, ok))| {
                let case = EvalCase {
                    id: (*id).into(),
                    split: Split::Test,
                    class: Some((*class).into()),
                    input: serde_json::Value::Null,
                    expected: serde_json::Value::Null,
                };
                CaseOutcome::new(&case, u32::try_from(i).unwrap(), *ok).with_metric("lat", 10.0)
            })
            .collect()
    }

    #[test]
    fn repeats_are_averaged_per_case() {
        let o = outcomes(&[("a", "x", true), ("a", "x", false), ("b", "y", true)]);
        let means = per_case_means(&o);
        assert_eq!(means["a"].metrics[PASS_RATE], 0.5);
        let agg = aggregate(&o, &BootstrapConfig::default());
        assert_eq!(agg.n_cases, 2);
        assert_eq!(agg.repeats, 2);
        assert_eq!(agg.metrics[PASS_RATE].mean, 0.75);
        assert_eq!(agg.per_class["y"].metrics[PASS_RATE].mean, 1.0);
    }

    #[test]
    fn thresholds_use_conservative_bound_and_per_class() {
        let o = outcomes(&[("a", "x", true), ("b", "x", true), ("c", "y", false)]);
        let agg = aggregate(&o, &BootstrapConfig::default());
        let th = |rule| Threshold {
            id: "T".into(),
            rule,
            description: String::new(),
        };
        let per_class = th(ThresholdRule::Metric {
            metric: PASS_RATE.into(),
            op: Comparison::Ge,
            value: 0.9,
            per_class: true,
            point: false,
        });
        let r = check_thresholds(&agg, &[per_class]);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].status, ThresholdStatus::Passed);
        assert_eq!(r[1].status, ThresholdStatus::Failed);
        let le = th(ThresholdRule::Metric {
            metric: "lat".into(),
            op: Comparison::Le,
            value: 10.0,
            per_class: false,
            point: false,
        });
        let missing = th(ThresholdRule::Metric {
            metric: "brak".into(),
            op: Comparison::Ge,
            value: 0.0,
            per_class: false,
            point: true,
        });
        let text = th(ThresholdRule::Text { text: "x".into() });
        let r = check_thresholds(&agg, &[le, missing, text]);
        let statuses: Vec<_> = r.iter().map(|x| x.status).collect();
        assert_eq!(
            statuses,
            [
                ThresholdStatus::Passed,
                ThresholdStatus::Failed,
                ThresholdStatus::NotEvaluated
            ]
        );
    }
}
