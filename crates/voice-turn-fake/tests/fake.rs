//! Testy atrap: kontrakt współdzielony + skrypty.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use voice_turn_contract::contract_tests::{self, poll_end};
use voice_turn_contract::{TurnDetector, TurnError, TurnEvent, TurnModel, TurnModelInput};
use voice_turn_fake::{ScriptedTurnDetector, ScriptedTurnModel};

#[test]
fn contract_suite() {
    contract_tests::run_all(ScriptedTurnDetector::new);
}

#[test]
fn annotated_delays_are_used_in_order() {
    let mut d = ScriptedTurnDetector::new();
    d.push_delay(700);
    d.push_delay(10);
    d.observe(&TurnEvent::SpeechStart { at_ms: 0 });
    d.observe(&TurnEvent::SpeechEnd { at_ms: 1000 });
    assert_eq!(poll_end(&mut d, 1000, 3000, 10), Some(1700));
    d.observe(&TurnEvent::SpeechStart { at_ms: 2000 });
    d.observe(&TurnEvent::SpeechEnd { at_ms: 2500 });
    // 10 ms przycięte do minimalnej ciszy (200 ms).
    assert_eq!(poll_end(&mut d, 2500, 5000, 10), Some(2700));
    assert!(d.decisions() > 0);
}

#[test]
fn scripted_model_queue_then_default() {
    let m = ScriptedTurnModel::constant(0.4);
    m.push(Ok(0.9));
    m.push(Err(TurnError::Model {
        reason: "brak modelu".into(),
    }));
    let input = TurnModelInput {
        audio: None,
        partial_text: Some("a"),
        silence_ms: 0,
    };
    assert_eq!(m.end_probability(&input), Ok(0.9));
    assert!(m.end_probability(&input).is_err());
    assert_eq!(m.end_probability(&input), Ok(0.4));
    assert_eq!(m.calls().len(), 3);
    assert_eq!(m.name(), "scripted");
}
