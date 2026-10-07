//! Protokół IPC przez transport w pamięci: uwierzytelnienie poświadczeniem, autoryzacja ról,
//! źródło zmian ustalane z roli, Broker-UI jako jedyny kanał zatwierdzeń, brak nasłuchu TCP.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use safety_broker_contract::contract_tests::{delta, host, request};
use safety_broker_contract::ipc::{ClientRole, Hello, ProofWire, Request, Response, UserChannel};
use safety_broker_contract::{
    ApprovalDecision, AutonomyLevel, AutonomyTarget, BrokerError, Capability, CommandOrigin,
    Decision, InputSource, KernelRule,
};
use safety_broker_impl::BrokerEngine;
use safety_broker_impl::ipc::{BrokerClient, BrokerServer, IpcError, in_memory_pair};
use tokio::io::{AsyncWriteExt, DuplexStream};
use watchdog_contract::{Clock, KillReason, ManualClock};

async fn connect(
    engine: &Arc<BrokerEngine>,
    role: ClientRole,
    id: &str,
) -> Result<BrokerClient<DuplexStream>, IpcError> {
    let credential = engine.issue_client_credential(id, role, 60_000);
    connect_with(
        engine,
        Hello {
            protocol: 1,
            credential,
            pid: 4242,
            sid: None,
            image: None,
        },
    )
    .await
}

async fn connect_with(
    engine: &Arc<BrokerEngine>,
    hello: Hello,
) -> Result<BrokerClient<DuplexStream>, IpcError> {
    let (client, server) = in_memory_pair();
    let srv = BrokerServer::new(engine.clone());
    tokio::spawn(async move {
        let _ = srv.serve(server).await;
    });
    BrokerClient::connect(client, hello).await
}

fn setup() -> (Arc<BrokerEngine>, Arc<ManualClock>) {
    let (e, _, clock) = common::engine();
    (Arc::new(e), clock)
}

#[tokio::test]
async fn credentials_are_checked() {
    let (e, clock) = setup();
    let mut cred = e.issue_client_credential("agent-1", ClientRole::Agent, 1_000);
    cred.role = ClientRole::BrokerUi;
    let hello = |credential| Hello {
        protocol: 1,
        credential,
        pid: 1,
        sid: None,
        image: None,
    };
    assert!(matches!(
        connect_with(&e, hello(cred)).await,
        Err(IpcError::Rejected(_))
    ));
    let mut cred = e.issue_client_credential("agent-1", ClientRole::Agent, 1_000);
    cred.mac.replace_range(0..2, "00");
    assert!(connect_with(&e, hello(cred)).await.is_err());
    let cred = e.issue_client_credential("agent-1", ClientRole::Agent, 1_000);
    let mut old = Hello {
        protocol: 99,
        ..hello(cred.clone())
    };
    assert!(connect_with(&e, old.clone()).await.is_err());
    old.protocol = 1;
    clock.advance(1_000);
    assert!(
        connect_with(&e, old).await.is_err(),
        "poświadczenie wygasło"
    );
    let (other, _, _) = common::engine();
    let foreign = other.issue_client_credential("agent-1", ClientRole::Agent, 60_000);
    assert!(
        connect_with(&e, hello(foreign)).await.is_err(),
        "poświadczenie innego uruchomienia"
    );
}

#[tokio::test]
async fn agent_cannot_use_approval_channel_or_claim_user_origin() {
    let (e, _) = setup();
    let mut agent = connect(&e, ClientRole::Agent, "delta").await.unwrap();
    for req in [
        Request::PendingApprovals,
        Request::KillAll {
            reason: KillReason::Hotkey,
        },
        Request::RevokeHolder { holder: delta() },
        Request::Metrics,
    ] {
        let r = agent.call(req).await.unwrap();
        assert!(
            matches!(r, Response::Error(BrokerError::Unauthorized(_))),
            "{r:?}"
        );
    }
    let proof = ProofWire {
        approval: safety_broker_contract::ApprovalId(1),
        nonce: safety_broker_contract::Nonce([0; 16]),
        source: InputSource::MouseClick,
        injected: false,
        at_ms: 0,
    };
    let r = agent
        .call(Request::Resolve {
            id: proof.approval,
            decision: ApprovalDecision::Allow,
            proof,
        })
        .await
        .unwrap();
    assert!(matches!(r, Response::Error(BrokerError::Unauthorized(_))));
    let raise = Request::RequestAutonomy {
        target: AutonomyTarget::Global,
        level: AutonomyLevel::L4,
        until_ms: None,
        via: UserChannel::UserInterface,
    };
    let r = agent.call(raise).await.unwrap();
    assert_eq!(
        r,
        Response::Error(BrokerError::KernelBlock(KernelRule::SelfEscalation))
    );
    let r = agent
        .call(Request::RequestPolicy(Box::new(
            safety_broker_contract::contract_tests::test_policy(),
        )))
        .await
        .unwrap();
    assert_eq!(
        r,
        Response::Error(BrokerError::KernelBlock(KernelRule::KernelPolicyChange))
    );
}

#[tokio::test]
async fn full_approval_round_trip_over_ipc() {
    let (e, clock) = setup();
    let mut agent = connect(&e, ClientRole::Agent, "delta").await.unwrap();
    let mut ui = connect(&e, ClientRole::BrokerUi, "broker-ui")
        .await
        .unwrap();
    let mut core = connect(&e, ClientRole::Core, "core").await.unwrap();
    let r = agent
        .call(Request::Decide(request(
            &delta(),
            Capability::NetEgress(host("x.example.org")),
            CommandOrigin::UserText,
        )))
        .await
        .unwrap();
    let Response::Decision(Decision::NeedsApproval(ticket)) = r else {
        panic!("{r:?}")
    };
    let Response::Pending(pending) = ui.call(Request::PendingApprovals).await.unwrap() else {
        panic!()
    };
    let ch = pending
        .into_iter()
        .find(|c| c.request.id == ticket.id)
        .unwrap();
    let proof = ProofWire {
        approval: ticket.id,
        nonce: ch.nonce,
        source: InputSource::Keyboard,
        injected: false,
        at_ms: clock.now_ms(),
    };
    let r = ui
        .call(Request::Resolve {
            id: ticket.id,
            decision: ApprovalDecision::Allow,
            proof,
        })
        .await
        .unwrap();
    assert_eq!(r, Response::Ok);
    let r = agent
        .call(Request::ApprovalStatus {
            id: ticket.id,
            requester: delta(),
        })
        .await
        .unwrap();
    assert!(matches!(
        r,
        Response::Status(safety_broker_contract::ApprovalStatus::Approved { token: Some(_) })
    ));
    let raise = Request::RequestAutonomy {
        target: AutonomyTarget::Global,
        level: AutonomyLevel::L4,
        until_ms: None,
        via: UserChannel::UserVoice,
    };
    let Response::Approval(Some(id)) = core.call(raise).await.unwrap() else {
        panic!()
    };
    let Response::Pending(p) = ui.call(Request::PendingApprovals).await.unwrap() else {
        panic!()
    };
    assert!(p.iter().any(|c| c.request.id == id && c.request.non_voice));
    let event = core_bus_contract::Event::new(
        core_bus_contract::EventKind::Custom("cli.bridge.action".into()),
        core_bus_contract::Level::Info,
        serde_json::json!({ "verified": "niezależnie niezweryfikowane" }),
    );
    assert!(matches!(
        core.call(Request::AuditAppend(Box::new(event)))
            .await
            .unwrap(),
        Response::Audited { .. }
    ));
    let Response::Killed(report) = ui
        .call(Request::KillAll {
            reason: KillReason::TrayButton,
        })
        .await
        .unwrap()
    else {
        panic!()
    };
    assert!(report.tokens_revoked >= 1 && report.audited);
}

#[tokio::test]
async fn oversized_and_garbage_frames_close_connection() {
    let (e, _) = setup();
    let (mut client, server) = in_memory_pair();
    let srv = BrokerServer::new(e.clone());
    let task = tokio::spawn(async move { srv.serve(server).await });
    client
        .write_all(&(64 * 1024 * 1024u32).to_le_bytes())
        .await
        .unwrap();
    assert!(task.await.unwrap().is_err());
    let (mut client, server) = in_memory_pair();
    let srv = BrokerServer::new(e);
    let task = tokio::spawn(async move { srv.serve(server).await });
    client.write_all(&4u32.to_le_bytes()).await.unwrap();
    client.write_all(b"nope").await.unwrap();
    assert!(task.await.unwrap().is_err());
}

#[test]
fn no_network_listeners_in_broker_sources() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![src];
    let mut checked = 0;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for banned in [
                "TcpListener",
                "TcpStream",
                "UdpSocket",
                "std::net",
                "tokio::net",
            ] {
                assert!(!text.contains(banned), "{}: {banned}", path.display());
            }
            checked += 1;
        }
    }
    assert!(checked >= 10);
}
