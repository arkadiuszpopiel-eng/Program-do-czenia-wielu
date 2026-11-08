//! Atrapa przechodzi wspólne testy kontraktowe; zegar wirtualny i nagrane zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use skills_contract::contract_tests as ct;
use skills_contract::{ApprovalOrigin, OwnerApproval, SkillSource, Skills};
use skills_fake::FakeSkills;

fn fake() -> FakeSkills {
    FakeSkills::new(ct::sample_catalog())
}

#[tokio::test]
async fn contract_suite() {
    ct::lifecycle(&fake()).await;
    ct::quarantine(&fake()).await;
    ct::export_import(&fake(), &fake()).await;
}

#[tokio::test]
async fn virtual_clock_and_events() {
    let f = fake();
    f.advance(1_000);
    let r = f
        .propose(ct::sample_skill("1.0.0"), SkillSource::User)
        .await
        .unwrap();
    assert_eq!(r.proposed_at_ms, 1_000);
    f.advance(500);
    let a = OwnerApproval {
        origin: ApprovalOrigin::Ui,
        reviewed_hash: r.hash.clone(),
    };
    let inst = f.approve(&r.skill.id, &r.skill.version, a).await.unwrap();
    assert_eq!(inst.decided_at_ms, Some(1_500));
    let kinds: Vec<String> = f
        .events()
        .iter()
        .map(|e| e.kind.as_str().to_owned())
        .collect();
    assert_eq!(kinds, vec!["skills.proposed", "skills.installed"]);
}
