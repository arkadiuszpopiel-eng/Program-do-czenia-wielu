//! Testy kontraktowe przepływów: reguły Jądra na L4, głos, taint, poziomy autonomii i dowody,
//! zatwierdzenia jednorazowe.

use std::sync::Arc;

use core_bus_contract::SessionId;
use risk_classifier_contract::{AutonomyLevel, CommandOrigin, KernelRule, SttConfidence};
use watchdog_contract::ManualClock;

use super::{
    FullBroker, allowed, approve, challenge, delete_request, delta, host, needs_approval, proof,
    request, session_l4, test_policy, tree,
};
use crate::{
    AppSelector, ApprovalDecision, ApprovalStatus, AutonomyChangeRequest, AutonomyTarget,
    BrokerError, Capability, ChangeOrigin, Decision, DenyReason, Holder, TaintSource,
};

fn blocked(d: Result<Decision, BrokerError>, rule: KernelRule) {
    assert_eq!(d, Ok(Decision::Deny(DenyReason::KernelBlock(rule))));
}

/// Twarde reguły Jądra obowiązują na L4.
pub async fn kernel_blocks_on_l4<B: FullBroker>(b: &B, clock: &ManualClock) {
    session_l4(b, clock).await;
    let app =
        |a: &str| Capability::GuiControl(AppSelector::parse(a).unwrap_or_else(|e| panic!("{e}")));
    let t = CommandOrigin::UserText;
    blocked(
        b.decide(request(&delta(), app("alfa-broker-ui.exe"), t))
            .await,
        KernelRule::GuiControlOfKernelProcess,
    );
    let creds = Capability::FsRead(tree(r"%USERPROFILE%\.claude"));
    blocked(
        b.decide(request(&delta(), creds, t)).await,
        KernelRule::CredentialDenylist,
    );
    let mut shell = request(
        &delta(),
        Capability::ShellExec(tree(r"C:\Users\ala\proj")),
        t,
    );
    shell.facts.command = Some("wevtutil cl Security".into());
    blocked(b.decide(shell).await, KernelRule::AuditDisable);
    blocked(
        b.decide(request(
            &delta(),
            Capability::NetEgress(host("claude.ai")),
            t,
        ))
        .await,
        KernelRule::ProviderWebUi,
    );
    let audit = Capability::FsWrite(tree(r"C:\ProgramData\AlfaBroker"));
    blocked(
        b.decide(request(&delta(), audit, t)).await,
        KernelRule::KernelPolicyChange,
    );
    assert!(b.metrics().kernel_blocks >= 5);
}

/// Destrukcja zlecona głosem wymaga potwierdzenia nie-głosem także na L4.
pub async fn voice_destruction_asks_on_l4<B: FullBroker>(b: &B, clock: &ManualClock) {
    session_l4(b, clock).await;
    let voice = CommandOrigin::UserVoice {
        confidence: SttConfidence::from_permille(990),
        speaker_verified: true,
    };
    let ticket = needs_approval(
        b.decide(delete_request(&delta(), r"C:\Users\ala\Docs\x.txt", voice))
            .await,
    );
    assert!(ticket.non_voice);
    let text = delete_request(
        &delta(),
        r"C:\Users\ala\Docs\x.txt",
        CommandOrigin::UserText,
    );
    allowed(b.decide(text).await);
}

/// Sesja `tainted`: egress wymaga potwierdzenia także na L4; taint jest monotoniczny.
pub async fn tainted_egress_asks_on_l4<B: FullBroker>(b: &B, clock: &ManualClock) {
    session_l4(b, clock).await;
    let egress = || {
        request(
            &delta(),
            Capability::NetEgress(host("api.example.com")),
            CommandOrigin::Agent,
        )
    };
    allowed(b.decide(egress()).await);
    let s1 = SessionId::new("s1");
    b.report_untrusted_input(&s1, TaintSource::Web)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(b.session_security(&s1).tainted);
    needs_approval(b.decide(egress()).await);
    let other = Holder::agent("s2", "delta");
    let r = request(
        &other,
        Capability::NetEgress(host("api.example.com")),
        CommandOrigin::Agent,
    );
    assert!(!b.session_security(&SessionId::new("s2")).tainted);
    assert!(matches!(
        b.decide(r).await,
        Ok(Decision::NeedsApproval(_) | Decision::Allow(_))
    ));
}

/// Agentka nie podnosi własnego poziomu; obniżenie działa od razu.
pub async fn agent_cannot_raise_own_level<B: FullBroker>(b: &B) {
    let s1 = SessionId::new("s1");
    let target = AutonomyTarget::Session {
        session: s1.clone(),
    };
    let raise = AutonomyChangeRequest {
        target: target.clone(),
        level: AutonomyLevel::L4,
        until_ms: None,
        origin: ChangeOrigin::Agent("delta".into()),
    };
    assert_eq!(
        b.request_autonomy_change(raise).await,
        Err(BrokerError::KernelBlock(KernelRule::SelfEscalation))
    );
    assert_eq!(b.autonomy(&s1, None), AutonomyLevel::L3);
    assert!(
        b.pending().is_empty(),
        "agentka nie może nawet utworzyć prośby"
    );
    let lower = AutonomyChangeRequest {
        target,
        level: AutonomyLevel::L1,
        until_ms: None,
        origin: ChangeOrigin::Agent("delta".into()),
    };
    assert_eq!(b.request_autonomy_change(lower).await, Ok(None));
    assert_eq!(b.autonomy(&s1, None), AutonomyLevel::L1);
}

/// Podniesienie wymaga ważnego, świeżego, niewstrzykniętego dowodu z właściwym nonce.
pub async fn raise_requires_valid_proof<B: FullBroker>(b: &B, clock: &ManualClock) {
    use crate::{InputSource, Nonce, broker_ui_only::physical_input_proof as mk};
    use watchdog_contract::Clock;
    let s1 = SessionId::new("s1");
    let req = || AutonomyChangeRequest {
        target: AutonomyTarget::Session {
            session: s1.clone(),
        },
        level: AutonomyLevel::L4,
        until_ms: None,
        origin: ChangeOrigin::UserInterface,
    };
    let new_id = || async {
        b.request_autonomy_change(req())
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| panic!("brak prośby"))
    };
    let id = new_id().await;
    let ch = challenge(b, id);
    let mut wrong = ch.nonce;
    wrong.0[0] ^= 1;
    let now = clock.now_ms();
    assert!(
        b.resolve(
            id,
            ApprovalDecision::Allow,
            mk(id, wrong, InputSource::MouseClick, false, now)
        )
        .await
        .is_err()
    );
    assert_eq!(b.autonomy(&s1, None), AutonomyLevel::L3);
    let id = new_id().await;
    let ch = challenge(b, id);
    assert!(
        b.resolve(id, ApprovalDecision::Allow, proof(&ch, clock, true))
            .await
            .is_err(),
        "wejście wstrzyknięte"
    );
    let id = new_id().await;
    let ch = challenge(b, id);
    let other = new_id().await;
    let foreign = mk(other, ch.nonce, InputSource::Keyboard, false, now);
    assert!(
        b.resolve(id, ApprovalDecision::Allow, foreign)
            .await
            .is_err(),
        "dowód innej prośby"
    );
    let id = new_id().await;
    let ch = challenge(b, id);
    clock.advance(test_policy().approval_ttl_ms + 1);
    assert!(
        b.resolve(id, ApprovalDecision::Allow, proof(&ch, clock, false))
            .await
            .is_err(),
        "po terminie"
    );
    assert_eq!(b.autonomy(&s1, None), AutonomyLevel::L3);
    let id = new_id().await;
    let ch = challenge(b, id);
    b.resolve(id, ApprovalDecision::Allow, proof(&ch, clock, false))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(b.autonomy(&s1, None), AutonomyLevel::L4);
    let replay = mk(id, ch.nonce, InputSource::MouseClick, false, clock.now_ms());
    assert!(
        b.resolve(id, ApprovalDecision::Allow, replay)
            .await
            .is_err(),
        "ponowne użycie nonce"
    );
    let _ = Nonce([0; 16]);
}

/// Zatwierdzona akcja: token wydany raz i tylko proszącemu.
pub async fn approval_flow<B: FullBroker>(b: &B, clock: &ManualClock) {
    let r = request(
        &delta(),
        Capability::NetEgress(host("upload.example.org")),
        CommandOrigin::UserText,
    );
    let ticket = needs_approval(b.decide(r).await);
    assert_eq!(
        b.approval_status(ticket.id, &delta()),
        Ok(ApprovalStatus::Pending)
    );
    let stranger = Holder::agent("s1", "beta");
    assert!(b.approval_status(ticket.id, &stranger).is_err());
    approve(b, clock, ticket.id, ApprovalDecision::Allow).await;
    let token = match b.approval_status(ticket.id, &delta()) {
        Ok(ApprovalStatus::Approved { token: Some(t) }) => t,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        b.approval_status(ticket.id, &delta()),
        Ok(ApprovalStatus::Approved { token: None })
    );
    let needed = Capability::NetEgress(host("upload.example.org"));
    assert_eq!(b.verify(&token, &needed, &delta()), Ok(()));
    let r = request(
        &delta(),
        Capability::NetEgress(host("x.example.org")),
        CommandOrigin::UserText,
    );
    let ticket = needs_approval(b.decide(r).await);
    approve(b, clock, ticket.id, ApprovalDecision::Deny).await;
    assert_eq!(
        b.approval_status(ticket.id, &delta()),
        Ok(ApprovalStatus::Denied)
    );
}

/// Uruchamia testy przepływów.
pub async fn run<B, F>(fresh: &F)
where
    B: FullBroker,
    F: Fn() -> (B, Arc<ManualClock>),
{
    let (b, c) = fresh();
    kernel_blocks_on_l4(&b, &c).await;
    let (b, c) = fresh();
    voice_destruction_asks_on_l4(&b, &c).await;
    let (b, c) = fresh();
    tainted_egress_asks_on_l4(&b, &c).await;
    agent_cannot_raise_own_level(&fresh().0).await;
    let (b, c) = fresh();
    raise_requires_valid_proof(&b, &c).await;
    let (b, c) = fresh();
    approval_flow(&b, &c).await;
}
