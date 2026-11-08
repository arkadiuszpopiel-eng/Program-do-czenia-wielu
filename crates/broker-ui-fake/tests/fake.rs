//! Atrapa: kontrakt współdzielony, skrypt per prośba i FIFO, wstrzyknięcie bez decyzji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use broker_ui_contract::contract_tests::{self, challenge, decision_matches};
use broker_ui_contract::{BrokerUi, RejectReason, UiEvent};
use broker_ui_fake::{Script, ScriptedBrokerUi};
use safety_broker_contract::{ApprovalDecision, ApprovalId};

#[test]
fn contract_suite() {
    contract_tests::queue_semantics(&mut ScriptedBrokerUi::new(), 1_000);
    contract_tests::expired_challenge_rejected(&mut ScriptedBrokerUi::new(), 1_000);
}

#[test]
fn scripted_decisions_carry_valid_proofs() {
    let mut ui = ScriptedBrokerUi::new();
    let (a, b, c) = (
        challenge(1, 1_000),
        challenge(2, 1_000),
        challenge(3, 1_000),
    );
    for ch in [&a, &b, &c] {
        ui.show(ch.clone(), 1_000).unwrap();
    }
    ui.script_for(ApprovalId(1), Script::Injected);
    ui.push_script(Script::AllowInScope { hours: 99 });
    assert!(
        ui.poll_decision(1_500, 0).is_none(),
        "wstrzyknięcie nie daje decyzji"
    );
    assert!(ui.drain_events().iter().any(|e| matches!(
        e,
        UiEvent::InputRejected {
            reason: RejectReason::Injected,
            ..
        }
    )));
    let d = ui.poll_decision(1_500, 0).unwrap();
    assert!(decision_matches(&d, &a).is_ok());
    assert!(
        matches!(d.decision, ApprovalDecision::AllowInScope { until_ms, .. } if until_ms == 1_500 + 24 * 3_600_000)
    );
    assert!(ui.poll_decision(1_500, 0).is_none(), "brak skryptu = czeka");
    ui.set_default(Some(Script::Deny));
    let d = ui.poll_decision(2_000, 0).unwrap();
    assert_eq!(d.decision, ApprovalDecision::Deny);
    assert!(decision_matches(&d, &b).is_ok());
    ui.script_for(ApprovalId(3), Script::Allow);
    let d = ui.poll_decision(0, 0).unwrap();
    assert!(
        decision_matches(&d, &c).is_ok(),
        "chwila przycięta do okna ważności"
    );
    assert!(ui.queued().is_empty());
}
