//! Testy kontraktowe: „zawsze zezwalaj w tym zakresie”, plan do zatwierdzenia, polityki Jądra.

use std::sync::Arc;

use core_bus_contract::SessionId;
use risk_classifier_contract::{AutonomyLevel, CommandOrigin, KernelRule, SttConfidence};
use watchdog_contract::ManualClock;

use super::{
    FullBroker, allowed, approve, challenge, delete_request, delta, exact, host, needs_approval,
    proof, request, test_policy, tree,
};
use crate::{
    AppSelector, ApprovalDecision, AutonomyChangeRequest, AutonomyTarget, BrokerError, Capability,
    ChangeOrigin, DeclaredFacts, Holder, PlanDecision, PlanRequest, PlanStep, TaintSource,
};

/// „Zawsze zezwalaj w tym zakresie” nie zmienia poziomu i nie pokrywa reguł każdego poziomu.
pub async fn allow_in_scope_never_escalates<B: FullBroker>(b: &B, clock: &ManualClock) {
    let ask = |h: &str| {
        request(
            &delta(),
            Capability::NetEgress(host(h)),
            CommandOrigin::UserText,
        )
    };
    let ticket = needs_approval(b.decide(ask("a.example.org")).await);
    let scope = Capability::NetEgress(host("*.example.org"));
    let until_ms = u64::MAX;
    approve(
        b,
        clock,
        ticket.id,
        ApprovalDecision::AllowInScope { scope, until_ms },
    )
    .await;
    allowed(b.decide(ask("b.example.org")).await);
    assert_eq!(
        b.autonomy(
            &SessionId::new("s1"),
            Some(&core_bus_contract::AgentId::new("delta"))
        ),
        AutonomyLevel::L3
    );
    needs_approval(b.decide(ask("example.net")).await);
    b.report_untrusted_input(&SessionId::new("s1"), TaintSource::Email)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    needs_approval(b.decide(ask("c.example.org")).await);
    let voice = CommandOrigin::UserVoice {
        confidence: SttConfidence::from_permille(990),
        speaker_verified: true,
    };
    let t2 = needs_approval(
        b.decide(delete_request(&delta(), r"C:\Users\ala\Docs\y.txt", voice))
            .await,
    );
    let grant = ApprovalDecision::AllowInScope {
        scope: Capability::FsWrite(tree(r"C:\Users\ala")),
        until_ms,
    };
    let ch = challenge(b, t2.id);
    assert!(
        b.resolve(t2.id, grant, proof(&ch, clock, false))
            .await
            .is_err(),
        "głos nie jest grantable"
    );
}

/// Plan do zatwierdzenia: jedno zatwierdzenie, wiele akcji w zakresie; krok z blokadą → odrzucony.
pub async fn plan_one_approval_many_actions<B: FullBroker>(b: &B, clock: &ManualClock) {
    let mut facts = DeclaredFacts::new("tools-fs.move");
    facts.destructive = risk_classifier_contract::Destructiveness::Recoverable;
    facts.bulk = 14;
    let step = PlanStep {
        capability: Capability::FsWrite(tree(r"C:\Users\ala\Downloads")),
        facts: facts.clone(),
        description: "Przenieś 14 plików".into(),
    };
    let plan = PlanRequest {
        holder: delta(),
        title: "Porządki".into(),
        origin: CommandOrigin::UserText,
        steps: vec![step.clone()],
        ttl_ms: 3_600_000,
    };
    let s1 = SessionId::new("s1");
    let lower = AutonomyChangeRequest {
        target: AutonomyTarget::Session { session: s1 },
        level: AutonomyLevel::L2,
        until_ms: None,
        origin: ChangeOrigin::UserInterface,
    };
    assert_eq!(b.request_autonomy_change(lower).await, Ok(None));
    let ticket = match b.submit_plan(plan.clone()).await {
        Ok(PlanDecision::NeedsApproval(t)) => t,
        other => panic!("{other:?}"),
    };
    approve(b, clock, ticket.id, ApprovalDecision::Allow).await;
    for i in 0..14 {
        let mut r = delete_request(
            &delta(),
            &format!(r"C:\Users\ala\Downloads\f{i}.txt"),
            CommandOrigin::UserText,
        );
        r.facts.tool = "tools-fs.move".into();
        allowed(b.decide(r).await);
    }
    needs_approval(
        b.decide(delete_request(
            &delta(),
            r"C:\Users\ala\Docs\z.txt",
            CommandOrigin::UserText,
        ))
        .await,
    );
    let other = delete_request(
        &Holder::agent("s1", "beta"),
        r"C:\Users\ala\Downloads\f1.txt",
        CommandOrigin::UserText,
    );
    needs_approval(b.decide(other).await);
    let mut bad = plan;
    bad.steps.push(PlanStep {
        capability: Capability::GuiControl(
            AppSelector::parse("alfa-watchdog").unwrap_or_else(|e| panic!("{e}")),
        ),
        facts,
        description: "x".into(),
    });
    assert_eq!(
        b.submit_plan(bad).await,
        Ok(PlanDecision::Rejected {
            step: 1,
            rule: KernelRule::GuiControlOfKernelProcess
        })
    );
}

/// Polityki Jądra: agentka nie zmienia; właściciel — przez Broker-UI z dowodem.
pub async fn policy_change_only_with_proof<B: FullBroker>(b: &B, clock: &ManualClock) {
    let mut policy = test_policy();
    policy.egress_allowlist.push(host("files.example.net"));
    let agent = ChangeOrigin::Agent("delta".into());
    assert_eq!(
        b.request_policy_change(policy.clone(), agent).await,
        Err(BrokerError::KernelBlock(KernelRule::KernelPolicyChange))
    );
    let r = || {
        request(
            &delta(),
            Capability::NetEgress(host("files.example.net")),
            CommandOrigin::UserText,
        )
    };
    needs_approval(b.decide(r()).await);
    let id = b
        .request_policy_change(policy, ChangeOrigin::UserInterface)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    approve(b, clock, id, ApprovalDecision::Allow).await;
    allowed(b.decide(r()).await);
    let mut broken = test_policy();
    broken.deny_lists.path_segments.clear();
    assert!(
        b.request_policy_change(broken, ChangeOrigin::UserInterface)
            .await
            .is_err()
    );
    let _ = exact(r"C:\Users\ala\x");
}

/// Uruchamia testy zgód zakresowych, planów i polityk.
pub async fn run<B, F>(fresh: &F)
where
    B: FullBroker,
    F: Fn() -> (B, Arc<ManualClock>),
{
    let (b, c) = fresh();
    allow_in_scope_never_escalates(&b, &c).await;
    let (b, c) = fresh();
    plan_one_approval_many_actions(&b, &c).await;
    let (b, c) = fresh();
    policy_change_only_with_proof(&b, &c).await;
}
