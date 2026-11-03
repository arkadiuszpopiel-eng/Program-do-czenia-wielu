//! Okno Sędziego: „Zezwól i przełącz na „Automatycznie”” (plan 50 §4.2, Z.3) — przycisk w rzędzie
//! głównym tylko przy prośbie z `auto_switch` (pojedyncza akcja, tryb globalny niższy niż L3),
//! poza rzędem czasów zgody; przycisk daje decyzję `AllowAndAutomatic`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use broker_ui_contract::{ApprovalCard, CardOptions, DecisionOption};
use broker_ui_impl::{BTN_AUTO, BTN_DENY, BTN_ONCE, BTN_SCOPED, decision_for, view_of};
use risk_classifier_contract::{AutonomyLevel, CommandOrigin, Reversibility, RiskLevel};
use safety_broker_contract::contract_tests::exact;
use safety_broker_contract::{
    ApprovalDecision, ApprovalId, ApprovalRequest, ApprovalSubject, Capability, Holder,
    PlanStepSummary,
};

fn request(subject: ApprovalSubject, auto_switch: bool) -> ApprovalRequest {
    ApprovalRequest {
        id: ApprovalId(3),
        holder: Holder::agent("s1", "delta"),
        subject,
        risk: RiskLevel::Medium,
        reversible: Reversibility::Yes,
        origin: CommandOrigin::Agent,
        tainted: false,
        non_voice: false,
        grantable: true,
        hello_required: false,
        rules: Vec::new(),
        autonomy: Some(AutonomyLevel::L1),
        auto_switch,
        explanation: String::new(),
        created_at_ms: 1_000,
        expires_at_ms: 601_000,
    }
}

fn write() -> Capability {
    Capability::FsWrite(exact(r"C:\Users\Ala\Documents\raport.txt"))
}

fn action() -> ApprovalSubject {
    ApprovalSubject::Action {
        capability: write(),
        tool: "tools-fs.write".into(),
        summary: Some("zapis „raport.txt”".into()),
    }
}

#[test]
fn switch_button_sits_in_the_main_row_and_maps_to_its_decision() {
    let card = ApprovalCard::from_request(&request(action(), true), 1_000, CardOptions::default());
    assert_eq!(
        &card.options[..3],
        &[
            DecisionOption::Deny,
            DecisionOption::AllowOnce,
            DecisionOption::AllowAndAutomatic
        ]
    );
    let view = view_of(&card, "", 1_000);
    let ids: Vec<u16> = view.buttons.iter().map(|b| b.id).collect();
    assert_eq!(&ids[..3], &[BTN_DENY, BTN_ONCE, BTN_AUTO]);
    let label = &view.buttons[2].label;
    assert_eq!(label, "Zezwól i przełącz na „Automatycznie”");
    let chips = &view.choice.as_ref().unwrap().buttons;
    assert!(!chips.contains(&BTN_AUTO), "poza rzędem czasów zgody");
    assert!(chips.iter().all(|id| *id >= BTN_SCOPED));
    assert_eq!(view.initial_focus, BTN_DENY);
    assert_eq!(
        decision_for(&card, BTN_AUTO),
        Some(ApprovalDecision::AllowAndAutomatic)
    );
}

#[test]
fn no_switch_without_the_flag_or_for_plans() {
    let card = ApprovalCard::from_request(&request(action(), false), 1_000, CardOptions::default());
    assert!(!card.options.contains(&DecisionOption::AllowAndAutomatic));
    assert_eq!(decision_for(&card, BTN_AUTO), None);

    let plan = ApprovalSubject::Plan {
        title: "Porządki".into(),
        steps: vec![PlanStepSummary {
            description: "zapis".into(),
            capability: write(),
            risk: RiskLevel::Low,
        }],
    };
    let card = ApprovalCard::from_request(&request(plan, true), 1_000, CardOptions::default());
    assert!(!card.options.contains(&DecisionOption::AllowAndAutomatic));
}
