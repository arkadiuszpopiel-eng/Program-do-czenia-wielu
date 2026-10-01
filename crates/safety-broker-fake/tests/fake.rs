//! Testy atrapy: kontrakt współdzielony, skrypt, nienaruszalność blokad Jądra, auto-zatwierdzanie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::PathEnv;
use safety_broker_contract::contract_tests::{self, PROFILE, delta, host, request, tree};
use safety_broker_contract::{
    AppSelector, ApprovalDecision, ApprovalStatus, Broker, Capability, CommandOrigin, Decision,
    DenyReason, KernelRule,
};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use watchdog_contract::{KillReason, KillSwitch};

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|policy, clock| {
        FakeBroker::with(policy, PathEnv::windows_profile(PROFILE), clock).unwrap()
    })
    .await;
}

fn fake() -> FakeBroker {
    FakeBroker::with(
        contract_tests::test_policy(),
        PathEnv::windows_profile(PROFILE),
        std::sync::Arc::new(watchdog_contract::ManualClock::new(1_000_000)),
    )
    .unwrap()
}

#[tokio::test]
async fn scripts_change_decisions_but_not_kernel_blocks() {
    let b = fake();
    let mut egress = request(
        &delta(),
        Capability::NetEgress(host("x.example.org")),
        CommandOrigin::UserText,
    );
    egress.facts.tool = "tools-net.post".into();
    assert!(matches!(
        b.decide(egress.clone()).await,
        Ok(Decision::NeedsApproval(_))
    ));
    b.script("tools-net.post", ScriptedDecision::Allow);
    assert!(matches!(
        b.decide(egress.clone()).await,
        Ok(Decision::Allow(_))
    ));
    b.script(
        "tools-net.post",
        ScriptedDecision::Deny(KernelRule::ProviderWebUi),
    );
    assert_eq!(
        b.decide(egress).await,
        Ok(Decision::Deny(DenyReason::KernelBlock(
            KernelRule::ProviderWebUi
        )))
    );
    let mut gui = request(
        &delta(),
        Capability::GuiControl(AppSelector::parse("alfa-broker-ui").unwrap()),
        CommandOrigin::UserText,
    );
    gui.facts.tool = "tools-gui.click".into();
    b.script("tools-gui.click", ScriptedDecision::Allow);
    assert_eq!(
        b.decide(gui).await,
        Ok(Decision::Deny(DenyReason::KernelBlock(
            KernelRule::GuiControlOfKernelProcess
        )))
    );
    let mut read = request(
        &delta(),
        Capability::FsRead(tree(r"C:\Users\ala\Docs")),
        CommandOrigin::UserText,
    );
    read.facts.tool = "tools-fs.read".into();
    b.script("tools-fs.read", ScriptedDecision::NeedsApproval);
    assert!(matches!(
        b.decide(read).await,
        Ok(Decision::NeedsApproval(_))
    ));
}

#[tokio::test]
async fn auto_approve_and_recorders() {
    let b = fake();
    let r = request(
        &delta(),
        Capability::NetEgress(host("x.example.org")),
        CommandOrigin::UserText,
    );
    let Ok(Decision::NeedsApproval(t)) = b.decide(r).await else {
        panic!()
    };
    assert_eq!(b.auto_approve(ApprovalDecision::Allow).await, 1);
    assert!(matches!(
        b.approval_status(t.id, &delta()),
        Ok(ApprovalStatus::Approved { token: Some(_) })
    ));
    assert!(
        b.audit_names()
            .iter()
            .any(|n| n == "broker.approval.decided")
    );
    assert!(!b.audit_events().is_empty());
    use watchdog_contract::JobRegistry;
    b.register_job(
        platform_contract::ProcessHandle(5),
        watchdog_contract::ProcessRole::Core,
        "core",
    );
    let report = b.kill_all(KillReason::VoiceStop).await;
    assert!(report.audio_silenced);
    assert_eq!(b.killed_jobs(), vec![5]);
    assert_eq!(b.silences(), vec![KillReason::VoiceStop]);
    assert!(FakeBroker::new().is_ok());
    assert_eq!(
        watchdog_contract::Clock::now_ms(b.clock().as_ref()),
        1_000_000
    );
}
