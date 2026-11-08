//! Właściwości detektora na losowych sekwencjach zdarzeń (wirtualny zegar, stałe ziarno).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use voice_turn_contract::{Patience, TurnDecision, TurnDetector, TurnEvent};
use voice_turn_impl::{HeuristicTurnModel, PatienceTurnDetector};

#[derive(Debug, Clone)]
enum Step {
    Start,
    End,
    Partial(usize),
    Tick(u64),
    Reset,
}

const TEXTS: &[&str] = &[
    "Hej.",
    "no i",
    "yyy",
    "Która godzina?",
    "otwórz pocztę",
    "że",
    "",
];

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        Just(Step::Start),
        Just(Step::End),
        (0..TEXTS.len()).prop_map(Step::Partial),
        (1u64..400).prop_map(Step::Tick),
        Just(Step::Reset),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, failure_persistence: None, rng_seed: proptest::test_runner::RngSeed::Fixed(0x7A11), ..ProptestConfig::default() })]

    /// Koniec tury nigdy w trakcie mowy, zawsze w [min_silence, max] od końca mowy,
    /// najwyżej raz na turę; `Wait.until` zawsze w przyszłości.
    #[test]
    fn invariants_hold(steps in prop::collection::vec(step(), 1..80)) {
        let cfg = Patience::Normal.cfg();
        let mut d = PatienceTurnDetector::new(HeuristicTurnModel);
        let mut now = 0u64;
        let mut speaking = false;
        let mut last_end: Option<u64> = None;
        let mut ended_this_turn = false;
        for s in steps {
            match s {
                Step::Start => { d.observe(&TurnEvent::SpeechStart { at_ms: now }); speaking = true; ended_this_turn = false; }
                Step::End => { if speaking || !ended_this_turn { d.observe(&TurnEvent::SpeechEnd { at_ms: now }); if speaking { last_end = Some(now); } speaking = false; } }
                Step::Partial(i) => d.observe(&TurnEvent::Partial { at_ms: now, text: TEXTS[i].into() }),
                Step::Tick(dt) => now += dt,
                Step::Reset => { d.observe(&TurnEvent::Reset); speaking = false; last_end = None; ended_this_turn = false; }
            }
            match d.decide(now, None) {
                TurnDecision::EndOfTurn { at_ms, .. } => {
                    prop_assert!(!speaking, "koniec tury w trakcie mowy");
                    prop_assert!(!ended_this_turn, "drugi koniec tej samej tury");
                    let end = last_end.unwrap();
                    prop_assert!(at_ms >= end + cfg.min_silence_ms);
                    ended_this_turn = true;
                }
                TurnDecision::Wait { until_ms, .. } => {
                    prop_assert!(until_ms >= now);
                    if let Some(end) = last_end.filter(|_| !speaking) {
                        prop_assert!(until_ms <= end + cfg.max_ms, "czekanie ponad twardy limit");
                    }
                }
                TurnDecision::Idle => {}
            }
        }
    }
}
