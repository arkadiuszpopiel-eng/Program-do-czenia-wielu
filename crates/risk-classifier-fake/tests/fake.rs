//! Testy atrapy: kontrakt współdzielony, skrypt per narzędzie, nienaruszalność twardych blokad.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use risk_classifier_contract::{
    ActionClass, ActionFacts, AutonomyLevel, KernelRule, RiskClassifier, RiskLevel, RiskPolicy,
    Verdict, contract_tests,
};
use risk_classifier_fake::FakeClassifier;

#[test]
fn contract_suite() {
    contract_tests::run_all(&FakeClassifier::new());
    contract_tests::run_all(&FakeClassifier::with_policy(RiskPolicy::default()));
}

#[test]
fn scripted_verdicts_and_calls() {
    let fake = FakeClassifier::new();
    let f = ActionFacts::new("tools-fs.write", ActionClass::Write);
    assert_eq!(fake.evaluate(&f, AutonomyLevel::L1).verdict.strictness(), 1);
    fake.script("tools-fs.write", RiskLevel::High, Verdict::Proceed);
    let v = fake.evaluate(&f, AutonomyLevel::L1);
    assert_eq!((v.level, v.verdict), (RiskLevel::High, Verdict::Proceed));
    assert!(v.explanation.contains("Atrapa"));
    fake.clear_script();
    assert_eq!(fake.evaluate(&f, AutonomyLevel::L1).verdict.strictness(), 1);
    assert_eq!(fake.calls().len(), 3);
}

#[test]
fn kernel_blocks_cannot_be_scripted_away() {
    let fake = FakeClassifier::new();
    fake.script("tools-shell.run", RiskLevel::Low, Verdict::Proceed);
    let f =
        ActionFacts::new("tools-shell.run", ActionClass::Shell).kernel(KernelRule::AuditDisable);
    for level in AutonomyLevel::ALL {
        assert_eq!(
            fake.evaluate(&f, level).verdict,
            Verdict::HardBlock {
                rule: KernelRule::AuditDisable
            }
        );
    }
}
