//! Raport przebiegu: JSON (artefakt CI) i Markdown (PR, panel „Zdrowie systemu”).

use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::aggregate::{Aggregate, ThresholdResult, ThresholdStatus, aggregate, check_thresholds};
use crate::case::CaseOutcome;
use crate::compare::VariantComparison;
use crate::gate::{GateDecision, GateVerdict};
use crate::integrity::IntegrityReport;
use crate::manifest::{Split, SuiteId, SuiteManifest, SuiteStatus};
use crate::stats::{BootstrapConfig, Interval};

/// Raport przebiegu jednego wariantu na podziale publicznym.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvalReport {
    /// Zestaw.
    pub suite: SuiteId,
    /// Hash manifestu.
    pub suite_digest: String,
    /// Status zestawu.
    pub status: SuiteStatus,
    /// Podział.
    pub split: Split,
    /// Wariant.
    pub variant: String,
    /// Czas wygenerowania (ms).
    pub generated_ms: u64,
    /// Agregat.
    pub aggregate: Aggregate,
    /// Progi.
    pub thresholds: Vec<ThresholdResult>,
    /// Integralność (jeśli sprawdzana).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<IntegrityReport>,
    /// Porównanie z wariantem bazowym (jeśli dotyczy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<VariantComparison>,
}

/// Składa raport z wyników.
pub fn build_report(
    manifest: &SuiteManifest,
    split: Split,
    variant: &str,
    outcomes: &[CaseOutcome],
    cfg: &BootstrapConfig,
    generated_ms: u64,
) -> EvalReport {
    let agg = aggregate(outcomes, cfg);
    EvalReport {
        suite: manifest.suite.clone(),
        suite_digest: manifest.digest(),
        status: manifest.status,
        split,
        variant: variant.to_owned(),
        generated_ms,
        thresholds: check_thresholds(&agg, &manifest.thresholds),
        aggregate: agg,
        integrity: None,
        comparison: None,
    }
}

fn fmt_interval(i: &Interval) -> String {
    format!("{:.3} [{:.3}; {:.3}]", i.mean, i.lo, i.hi)
}

fn status_pl(s: ThresholdStatus) -> &'static str {
    match s {
        ThresholdStatus::Passed => "spełniony",
        ThresholdStatus::Failed => "**NIESPEŁNIONY**",
        ThresholdStatus::NotEvaluated => "opisowy (test modułu / człowiek)",
    }
}

fn thresholds_md(out: &mut String, thresholds: &[ThresholdResult]) {
    if thresholds.is_empty() {
        return;
    }
    out.push_str("\n| Próg | Klasa | Obserwowane | Wymagane | Wynik |\n|---|---|---|---|---|\n");
    for t in thresholds {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            t.id,
            t.class.as_deref().unwrap_or("—"),
            t.observed
                .map_or_else(|| "—".to_owned(), |v| format!("{v:.3}")),
            t.threshold
                .map_or_else(|| "—".to_owned(), |v| format!("{v}")),
            status_pl(t.status)
        );
    }
}

impl EvalReport {
    /// JSON (ładny).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Markdown po polsku.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# Raport ewaluacji `{}` — wariant `{}`\n\nPodział: `{}` · przypadków: {} · powtórzeń: {} · manifest: `{}`\n",
            self.suite,
            self.variant,
            self.split.as_str(),
            self.aggregate.n_cases,
            self.aggregate.repeats,
            self.suite_digest.chars().take(16).collect::<String>()
        );
        out.push_str("| Metryka | Średnia [95% CI] |\n|---|---|\n");
        for (name, i) in &self.aggregate.metrics {
            let _ = writeln!(out, "| {name} | {} |", fmt_interval(i));
        }
        if !self.aggregate.per_class.is_empty() {
            out.push_str("\n| Klasa | Przypadków | pass_rate |\n|---|---|---|\n");
            for (class, c) in &self.aggregate.per_class {
                let pr = c
                    .metrics
                    .get(crate::aggregate::PASS_RATE)
                    .map_or_else(|| "—".to_owned(), fmt_interval);
                let _ = writeln!(out, "| {class} | {} | {pr} |", c.n_cases);
            }
        }
        thresholds_md(&mut out, &self.thresholds);
        if let Some(c) = &self.comparison {
            let _ = writeln!(
                out,
                "\nPorównanie `{}`: baza {} → kandydat {}; poprawa {} ({:?}).",
                c.metric,
                fmt_interval(&c.baseline),
                fmt_interval(&c.candidate),
                fmt_interval(&c.improvement),
                c.verdict
            );
        }
        if let Some(i) = &self.integrity {
            let state = if i.is_intact() {
                "zgodna"
            } else {
                "**NARUSZONA**"
            };
            let _ = writeln!(out, "\nIntegralność: {state} ({} plików).", i.checked);
        }
        out
    }
}

impl GateVerdict {
    /// Markdown werdyktu (wynik zbiorczy).
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let decision = match &self.decision {
            GateDecision::Pass => "PRZESZEDŁ".to_owned(),
            GateDecision::Fail { reasons } => format!("ODRZUCONY — {}", reasons.join("; ")),
        };
        let _ = writeln!(
            out,
            "# Bramka ewaluacyjna `{}` ({:?})\n\nPrzypadków: {} · powtórzeń: {} · metryka: `{}`\n\nDecyzja: **{decision}**\n\nPoprawa: {}\n",
            self.suite,
            self.stage,
            self.n_cases,
            self.repeats,
            self.primary_metric,
            fmt_interval(&self.improvement)
        );
        out.push_str("| Metryka | Baza | Kandydat |\n|---|---|---|\n");
        for (name, c) in &self.candidate.metrics {
            let b = self
                .baseline
                .metrics
                .get(name)
                .map_or_else(|| "—".to_owned(), fmt_interval);
            let _ = writeln!(out, "| {name} | {b} | {} |", fmt_interval(c));
        }
        thresholds_md(&mut out, &self.thresholds);
        out
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::case::EvalCase;
    use crate::manifest::{Comparison, Threshold, ThresholdRule};

    #[test]
    fn report_renders_json_and_markdown() {
        let manifest = SuiteManifest {
            schema: 1,
            suite: SuiteId::new("demo").unwrap(),
            wave: "F8".into(),
            version: 1,
            status: SuiteStatus::Proposed,
            created: "2026-10-01".into(),
            accepted_by: None,
            description: String::new(),
            files: BTreeMap::from([("F8/x".to_owned(), "0".repeat(64))]),
            cases: Vec::new(),
            thresholds: vec![Threshold {
                id: "F8-01".into(),
                rule: ThresholdRule::Metric {
                    metric: "pass_rate".into(),
                    op: Comparison::Ge,
                    value: 0.5,
                    per_class: false,
                    point: false,
                },
                description: String::new(),
            }],
        };
        let outcomes: Vec<CaseOutcome> = (0..10)
            .map(|i| {
                let case = EvalCase {
                    id: format!("c{i}"),
                    split: Split::Test,
                    class: Some(if i % 2 == 0 { "a" } else { "b" }.into()),
                    input: serde_json::Value::Null,
                    expected: serde_json::Value::Null,
                };
                CaseOutcome::new(&case, 0, true)
            })
            .collect();
        let report = build_report(
            &manifest,
            Split::Test,
            "baseline",
            &outcomes,
            &BootstrapConfig::default(),
            7,
        );
        assert_eq!(report.thresholds[0].status, ThresholdStatus::Passed);
        let back: EvalReport = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(back, report);
        let md = report.to_markdown();
        assert!(md.contains("Raport ewaluacji `demo`"));
        assert!(md.contains("| F8-01 |"));
        assert!(md.contains("| a | 5 |"));
    }
}
