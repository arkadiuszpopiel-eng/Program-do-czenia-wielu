//! Atrapa przechodzi wspólne testy kontraktowe: ścieżka szczęśliwa i ≥ 30 prób ataku = 0 sukcesów.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use agent_builder_contract::contract_tests as ct;
use agent_builder_contract::{AgentBuilder, BuildError, BuilderPolicy};
use agent_builder_fake::FakeAgentBuilder;
use risk_classifier_contract::AutonomyLevel;

fn fake() -> FakeAgentBuilder {
    FakeAgentBuilder::new(BuilderPolicy::default(), ct::sample_tools())
}

#[tokio::test]
async fn happy_path() {
    let b = fake();
    ct::happy_path(&b).await;
    let kinds: Vec<String> = b
        .events()
        .iter()
        .map(|e| e.kind.as_str().to_owned())
        .collect();
    assert_eq!(
        kinds.last().map(String::as_str),
        Some("agent_builder.saved")
    );
}

#[tokio::test]
async fn attacks_never_succeed() {
    let (tries, wins) = ct::attacks(&fake()).await;
    eprintln!("Kreator: {tries} prób ataku, {wins} sukcesów");
    assert!(tries >= ct::ATTACKS_MIN);
    assert_eq!(wins, 0);
}

#[tokio::test]
async fn ceiling_follows_creating_session() {
    let b = FakeAgentBuilder::new(
        BuilderPolicy::with_ceiling(AutonomyLevel::L1),
        ct::sample_tools(),
    );
    let mut d = b.propose(ct::DESCRIPTION).draft;
    let built = b.build(&d).unwrap();
    assert_eq!(
        built.manifest.limits.autonomy,
        AutonomyLevel::L1,
        "domyślnie = sufit sesji"
    );
    for l in [AutonomyLevel::L2, AutonomyLevel::L3, AutonomyLevel::L4] {
        d.limits.autonomy = Some(l);
        assert!(
            matches!(b.build(&d), Err(BuildError::AutonomyTooHigh { .. })),
            "{l:?}"
        );
    }
    let max = FakeAgentBuilder::new(
        BuilderPolicy::with_ceiling(AutonomyLevel::L4),
        ct::sample_tools(),
    );
    assert_eq!(
        max.policy().ceiling,
        AutonomyLevel::L3,
        "L4 nigdy z Kreatora"
    );
}
