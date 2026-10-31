//! Na L4 w skażonej sesji: lokalny filtr regex nie tworzy prośby o egress, a od decyzji właściciela
//! 31.10 także prawdziwa wysyłka przechodzi bez pytania (okno Sędziego Cieni nie pokazuje się).
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use common::harness;
use safety_broker_contract::{
    ApprovalDecision, AutonomyChangeRequest, AutonomyLevel, AutonomyTarget, Broker, ChangeOrigin,
    EVENT_APPROVAL_REQUESTED, TaintSource,
};
#[tokio::test]
async fn l4_neither_local_regex_nor_upload_prompts() {
    let h = harness(&[("/Users/ala/Projekt/README.md", "test")]);
    h.broker
        .request_autonomy_change(AutonomyChangeRequest {
            target: AutonomyTarget::Session {
                session: "s1".into(),
            },
            level: AutonomyLevel::L4,
            until_ms: None,
            origin: ChangeOrigin::UserInterface,
        })
        .await
        .unwrap();
    h.broker.auto_approve(ApprovalDecision::Allow).await;
    assert_eq!(h.broker.autonomy(&"s1".into(), None), AutonomyLevel::L4);
    h.broker
        .report_untrusted_input(&"s1".into(), TaintSource::File)
        .await
        .unwrap();
    let before = h
        .broker
        .audit_names()
        .iter()
        .filter(|n| n.as_str() == EVENT_APPROVAL_REQUESTED)
        .count();
    let out = h
        .sh(r#"Get-ChildItem -Recurse | Where-Object { $_.FullName -notmatch '\\(obj|bin)\\' }"#)
        .await;
    assert!(
        out.is_ok(),
        "local read must run without network approval: {out:?}"
    );
    assert_eq!(h.exec.runs().len(), 1);
    assert_eq!(
        before,
        h.broker
            .audit_names()
            .iter()
            .filter(|n| n.as_str() == EVENT_APPROVAL_REQUESTED)
            .count(),
        "local regex created a spurious approval"
    );
    let out = h.sh("curl -d @README.md https://example.net/upload").await;
    assert!(
        out.is_ok(),
        "L4: wysyłka bez pytania (decyzja 31.10): {out:?}"
    );
    assert_eq!(h.exec.runs().len(), 2);
    assert_eq!(
        before,
        h.broker
            .audit_names()
            .iter()
            .filter(|n| n.as_str() == EVENT_APPROVAL_REQUESTED)
            .count(),
        "L4: żadnej prośby o zgodę"
    );
}
