//! Kontrakt `BrokerUi` na natywnej implementacji, manifest, łącze przez named pipe (atrapa
//! potoków z serwerem protokołu w wątku) z kontrolą konta serwera.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;

use broker_ui_contract::contract_tests::{self, challenge};
use broker_ui_contract::{BrokerLink, NoHello, UiConfig};
use broker_ui_impl::{MODULE_TOML, NativeBrokerUi, PipeLink};
use core_registry_contract::{Isolation, ModuleManifest};
use platform_contract::{
    IntegrityLevel, PeerIdentity, PipeSecurity, SecurePipePort, Sid, SignatureStatus,
};
use platform_fake::{FakePipes, FakeSurface};
use safety_broker_contract::ipc::{
    ClientCredential, ClientRole, Envelope, Hello, HelloReply, Request, Response,
};
use safety_broker_contract::ipc_blocking::{UiLaunchTicket, read_frame, write_frame};
use safety_broker_contract::{ApprovalDecision, InputSource, broker_ui_only};

#[test]
fn contract_suite_on_native_ui() {
    let mk = || {
        NativeBrokerUi::new(
            Arc::new(FakeSurface::new()),
            Arc::new(NoHello),
            UiConfig::default(),
        )
    };
    contract_tests::queue_semantics(&mut mk(), 1_000);
    contract_tests::expired_challenge_rejected(&mut mk(), 1_000);
}

#[test]
fn manifest_is_valid() {
    let m = ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "broker-ui");
    assert_eq!(m.isolation, Isolation::Process);
    assert!(m.budget.ram_mb <= 8);
}

const BROKER: &str = "S-1-5-80-7-7-7-7-7";
const USER: &str = "S-1-5-21-1-2-3-1001";

fn ident(pid: u32, user: &str, integrity: IntegrityLevel) -> PeerIdentity {
    PeerIdentity {
        pid,
        image: PathBuf::from(format!(r"C:\Alfa\{pid}.exe")),
        user: Sid::parse(user).unwrap(),
        integrity,
        session: 1,
        signature: SignatureStatus::NotVerified,
    }
}

fn ticket(broker_user: &str) -> UiLaunchTicket {
    UiLaunchTicket {
        credential: ClientCredential {
            client_id: "broker-ui".into(),
            role: ClientRole::BrokerUi,
            expires_at_ms: u64::MAX,
            mac: "00".into(),
        },
        pipe: "alfa-broker-test".into(),
        broker_user: Some(broker_user.into()),
    }
}

#[test]
fn pipe_link_talks_protocol_and_checks_server_account() {
    let sys = FakePipes::new(ident(1, BROKER, IntegrityLevel::System));
    sys.register(ident(2, USER, IntegrityLevel::High));
    let sec = PipeSecurity::new(
        "alfa-broker-test",
        Sid::parse(BROKER).unwrap(),
        vec![Sid::parse(USER).unwrap()],
    )
    .unwrap();
    let mut listener = sys.listen(&sec).unwrap();
    let server = std::thread::spawn(move || {
        let mut conn = listener.accept().unwrap();
        let hello: Hello = read_frame(&mut conn).unwrap().unwrap();
        assert_eq!(hello.credential.role, ClientRole::BrokerUi);
        assert_eq!(hello.pid, 2);
        write_frame(&mut conn, &HelloReply::Welcome { protocol: 1 }).unwrap();
        let mut seen = Vec::new();
        while let Some(env) = read_frame::<_, Envelope<Request>>(&mut conn).unwrap() {
            let body = match &env.body {
                Request::PendingApprovals => Response::Pending(vec![challenge(4, 10)]),
                Request::Resolve { proof, .. } => {
                    assert!(!proof.injected);
                    Response::Ok
                }
                _ => Response::Ok,
            };
            seen.push(env.body);
            write_frame(
                &mut conn,
                &Envelope {
                    v: 1,
                    id: env.id,
                    body,
                },
            )
            .unwrap();
        }
        seen.len()
    });
    let client = sys.process(2);
    let mut link = PipeLink::connect(&client, &client, &ticket(BROKER), 2).unwrap();
    let pending = link.pending().unwrap();
    assert_eq!(pending.len(), 1);
    let ch = &pending[0];
    let proof = broker_ui_only::physical_input_proof(
        ch.request.id,
        ch.nonce,
        InputSource::Keyboard,
        false,
        20,
    );
    link.resolve(broker_ui_contract::UiDecision {
        id: ch.request.id,
        decision: ApprovalDecision::Deny,
        proof,
    })
    .unwrap();
    drop(link);
    assert_eq!(server.join().unwrap(), 2);

    let sec2 = PipeSecurity::new(
        "alfa-broker-test",
        Sid::parse(USER).unwrap(),
        vec![Sid::parse(USER).unwrap()],
    )
    .unwrap();
    let squatter = sys.process(2).listen(&sec2).unwrap();
    let err = PipeLink::connect(&client, &client, &ticket(BROKER), 2)
        .err()
        .unwrap();
    assert!(err.to_string().contains("możliwe podstawienie"), "{err}");
    drop(squatter);
}
