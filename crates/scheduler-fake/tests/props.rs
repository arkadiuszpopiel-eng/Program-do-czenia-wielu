//! F5-03 / ACC-F5-scheduler-01: 0 zakleszczeń w 1000 losowych scenariuszy (zestaw zamrożony:
//! `evals/F5/scheduler-scenarios.json` — liczba przypadków i ziarno). Niezmienniki w `common`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::scenario_gen::{Rng, scenario};

#[derive(serde::Deserialize)]
struct Set {
    cases: usize,
    master_seed: u64,
}

#[test]
fn zero_deadlocks_in_1000_random_scenarios() {
    let set: Set =
        serde_json::from_str(include_str!("../../../evals/F5/scheduler-scenarios.json")).unwrap();
    assert_eq!(set.cases, 1000);
    let mut seeds = Rng::new(set.master_seed);
    let mut failures = Vec::new();
    let (mut tasks, mut steers, mut voice) = (0, 0, 0);
    for case in 0..set.cases {
        let seed = seeds.next();
        let report = common::run(&scenario(seed));
        tasks += report.tasks;
        steers += report.steers_delivered;
        voice += report.voice_granted;
        if !report.violations.is_empty() {
            failures.push(format!(
                "przypadek {case} (ziarno {seed}): {:?}",
                report.violations
            ));
        }
    }
    eprintln!(
        "F5-03: {} scenariuszy, {tasks} zadań, {steers} dostarczonych sterowań, {voice} przyznań mowy, zakleszczeń/naruszeń: {}",
        set.cases,
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
