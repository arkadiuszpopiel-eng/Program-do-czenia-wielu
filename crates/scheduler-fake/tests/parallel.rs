//! F5-01 / ACC-F5-scheduler-02: agentki równolegle z blokadą ekranu/głośnika — 0 konfliktów
//! zasobów w 100 scenariuszach (zestaw: `evals/F5/parallel-scenarios.json`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::scenario_gen::{Rng, scenario_parallel};

#[derive(serde::Deserialize)]
struct Threshold {
    resource_conflicts: usize,
    min_scenarios_with_parallel_agents: usize,
}

#[derive(serde::Deserialize)]
struct Set {
    cases: usize,
    master_seed: u64,
    threshold: Threshold,
}

#[test]
fn parallel_agents_without_resource_conflicts() {
    let set: Set =
        serde_json::from_str(include_str!("../../../evals/F5/parallel-scenarios.json")).unwrap();
    let mut seeds = Rng::new(set.master_seed);
    let (mut conflicts, mut parallel) = (0usize, 0usize);
    let mut details = Vec::new();
    for case in 0..set.cases {
        let seed = seeds.next();
        let report = common::run(&scenario_parallel(seed));
        if report.max_parallel_agents >= 2 {
            parallel += 1;
        }
        let c: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.contains("naraz") || v.contains("trzyma zasoby"))
            .collect();
        conflicts += c.len();
        if !report.violations.is_empty() {
            details.push(format!("przypadek {case}: {:?}", report.violations));
        }
    }
    eprintln!(
        "F5-01: {} scenariuszy, równoległe agentki w {parallel}, konflikty zasobów: {conflicts}",
        set.cases
    );
    assert!(details.is_empty(), "{}", details.join("\n"));
    assert_eq!(conflicts, set.threshold.resource_conflicts);
    assert!(parallel >= set.threshold.min_scenarios_with_parallel_agents);
}
