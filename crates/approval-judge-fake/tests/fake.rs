//! Atrapa sędziego: kolejka, werdykt domyślny, rejestr wejść.

use approval_judge_contract::{
    ApprovalJudge, DenyCategory, JudgeInput, JudgeVerdict, JudgedAction,
};
use approval_judge_fake::ScriptedJudge;
use risk_classifier_contract::{Destructiveness, Reversibility, RuleId};

fn input(tool: &str) -> JudgeInput {
    JudgeInput {
        goal: "cel".into(),
        transcript: vec![],
        action: JudgedAction {
            tool: tool.into(),
            capability_summary: "zakres".into(),
            command: None,
            paths: vec![],
            host: None,
            reversible: Reversibility::Yes,
            destructive: Destructiveness::None,
            rules: vec![RuleId::OutsideWorkdir],
            outside_workdir: true,
            tainted: false,
        },
        limits: vec![],
    }
}

#[tokio::test]
async fn queue_then_default_then_escalate() {
    let judge = ScriptedJudge::with_queue([JudgeVerdict::Allow { reason: "a".into() }]);
    assert!(matches!(
        judge.review(&input("t1")).await,
        JudgeVerdict::Allow { .. }
    ));
    assert!(matches!(
        judge.review(&input("t2")).await,
        JudgeVerdict::Escalate { .. }
    ));
    judge.set_default(JudgeVerdict::Deny {
        category: DenyCategory::Other,
        reason: "d".into(),
    });
    assert!(judge.review(&input("t3")).await.is_deny());
    let seen: Vec<String> = judge.seen().into_iter().map(|i| i.action.tool).collect();
    assert_eq!(seen, ["t1", "t2", "t3"]);
    assert_eq!(judge.model_name(), "fake:scripted");
}
