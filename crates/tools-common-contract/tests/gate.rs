//! Protokół Brokera na atrapie z prawdziwym silnikiem: Allow → verify, NeedsApproval →
//! zatwierdzenie/odmowa/limit czasu/anulowanie, Deny (Jądro), authorize_all z wycofaniem.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use compliance_contract::PathEnv;
use risk_classifier_contract::KernelRule;
use safety_broker_contract::{
    ApprovalDecision, ApprovalId, ApprovalTicket, Broker, Capability, DeclaredFacts, Holder,
    KernelPolicy,
};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use tools_common_contract::{
    BrokerGate, DenialReason, GateError, ToolCtx, ToolObserver, action_request, paths,
};
use watchdog_contract::ManualClock;

fn broker() -> Arc<FakeBroker> {
    let env = PathEnv::windows_profile("/Users/ala");
    let policy = KernelPolicy::baseline("/Users/ala", "/ProgramData/AlfaBroker").unwrap();
    Arc::new(FakeBroker::with(policy, env, Arc::new(ManualClock::new(1_000_000))).unwrap())
}

fn read_cap(p: &str) -> Capability {
    Capability::FsRead(paths::exact_scope(p, &PathEnv::windows_profile("/Users/ala")).unwrap())
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta"));
    c.approval_timeout = Duration::from_secs(30);
    c
}

fn facts(tool: &str) -> DeclaredFacts {
    DeclaredFacts::new(tool)
}

#[derive(Default)]
struct Seen(Mutex<Vec<(ApprovalId, Option<bool>)>>);

impl ToolObserver for Seen {
    fn approval_requested(&self, t: &ApprovalTicket) {
        self.0.lock().unwrap().push((t.id, None));
    }
    fn approval_resolved(&self, id: ApprovalId, ok: bool) {
        self.0.lock().unwrap().push((id, Some(ok)));
    }
}

#[tokio::test]
async fn allow_then_verify_and_release() {
    let b = broker();
    let gate = BrokerGate::new(b.clone());
    let c = ctx();
    let cap = read_cap("/Users/ala/a.txt");
    let auth = gate
        .authorize(action_request(&c, cap.clone(), facts("t.read")), &c)
        .await
        .unwrap();
    assert!(auth.approval.is_none());
    gate.verify(&auth, &cap, &c.holder).unwrap();
    let other = read_cap("/Users/ala/b.txt");
    assert!(matches!(
        gate.verify(&auth, &other, &c.holder),
        Err(GateError::Denied(DenialReason::TokenRejected))
    ));
    gate.release(std::slice::from_ref(&auth)).await;
    assert!(gate.verify(&auth, &cap, &c.holder).is_err());
}

#[tokio::test]
async fn kernel_block_is_denied() {
    let b = broker();
    let gate = BrokerGate::new(b.clone());
    let c = ctx();
    let cap = read_cap("/Users/ala/.claude/.credentials.json");
    let err = gate
        .authorize(action_request(&c, cap, facts("t.read")), &c)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        GateError::Denied(DenialReason::KernelBlock {
            rule: KernelRule::CredentialDenylist
        })
    );
    let out = err.into_outcome("odczyt");
    assert!(out.text.contains("Odmowa") && out.text.contains("Jądra"));
}

#[tokio::test(start_paused = true)]
async fn approval_flows() {
    let b = broker();
    b.script("t.ask", ScriptedDecision::NeedsApproval);
    let gate = BrokerGate::new(b.clone()).with_poll(Duration::from_millis(10));
    let seen = Arc::new(Seen::default());
    let mut c = ctx();
    c.observer = Some(seen.clone());
    let cap = read_cap("/Users/ala/a.txt");

    // Zatwierdzenie w trakcie czekania.
    let b2 = b.clone();
    let approver = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        b2.auto_approve(ApprovalDecision::Allow).await
    });
    let auth = gate
        .authorize(action_request(&c, cap.clone(), facts("t.ask")), &c)
        .await
        .unwrap();
    assert_eq!(approver.await.unwrap(), 1);
    assert!(auth.approval.is_some());
    gate.verify(&auth, &cap, &c.holder).unwrap();

    // Odmowa właściciela.
    let b3 = b.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        b3.auto_approve(ApprovalDecision::Deny).await
    });
    let err = gate
        .authorize(action_request(&c, cap.clone(), facts("t.ask")), &c)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        GateError::Denied(DenialReason::OwnerDenied { .. })
    ));

    // Limit czasu.
    c.approval_timeout = Duration::from_millis(100);
    let err = gate
        .authorize(action_request(&c, cap.clone(), facts("t.ask")), &c)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        GateError::Denied(DenialReason::ApprovalTimeout { .. })
    ));

    // Anulowanie w trakcie czekania.
    c.approval_timeout = Duration::from_secs(60);
    let cancel = c.cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        cancel.cancel();
    });
    let err = gate
        .authorize(action_request(&c, cap.clone(), facts("t.ask")), &c)
        .await
        .unwrap_err();
    assert_eq!(err, GateError::Cancelled);
    assert_eq!(
        gate.authorize(action_request(&c, cap, facts("t.ask")), &c)
            .await
            .unwrap_err(),
        GateError::Cancelled
    );
    let s = seen.0.lock().unwrap();
    assert_eq!(s.iter().filter(|(_, r)| r.is_none()).count(), 4);
    assert_eq!(s.iter().filter(|(_, r)| *r == Some(true)).count(), 1);
}

#[tokio::test]
async fn authorize_all_revokes_on_failure() {
    let b = broker();
    let gate = BrokerGate::new(b.clone());
    let c = ctx();
    let ok_cap = read_cap("/Users/ala/a.txt");
    let bad = read_cap("/Users/ala/.codex/auth.json");
    let reqs = vec![
        action_request(&c, ok_cap.clone(), facts("t.read")),
        action_request(&c, bad, facts("t.read")),
    ];
    let err = gate.authorize_all(reqs, &c).await.unwrap_err();
    assert!(matches!(
        err,
        GateError::Denied(DenialReason::KernelBlock { .. })
    ));
    assert!(b.audit_names().iter().any(|n| n == "broker.token.revoked"));
    let good = gate
        .authorize_all(vec![action_request(&c, ok_cap, facts("t.read"))], &c)
        .await
        .unwrap();
    assert_eq!(good.len(), 1);
    assert!(format!("{gate:?}").contains("BrokerGate"));
    assert!(!b.session_security(&c.holder.session).tainted);
}
