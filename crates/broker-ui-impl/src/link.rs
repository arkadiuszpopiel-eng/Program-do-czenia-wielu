//! Łącza z Brokerem: wewnątrzprocesowe (`ApprovalChannel`, testy i tryb deweloperski) oraz
//! named pipe z ACL (produkcja): bilet startowy ze stdin, sprawdzenie konta serwera potoku
//! (ochrona przed podstawionym serwerem), powitanie z poświadczeniem roli `BrokerUi`.

use std::sync::Arc;

use broker_ui_contract::{BrokerLink, UiDecision, UiError};
use platform_contract::{PipeConnection, ProcessIdentityPort, SecurePipePort};
use safety_broker_contract::ipc::{Hello, PROTOCOL_VERSION, Request, Response};
use safety_broker_contract::ipc_blocking::{BlockingClient, UiLaunchTicket};
use safety_broker_contract::{ApprovalChallenge, ApprovalChannel};

/// Łącze wewnątrzprocesowe do kanału zatwierdzeń.
pub struct ChannelLink<A: ApprovalChannel + ?Sized> {
    channel: Arc<A>,
    runtime: tokio::runtime::Runtime,
}

impl<A: ApprovalChannel + ?Sized> ChannelLink<A> {
    /// Łącze z własnym jednowątkowym runtime (nie wołać z wnętrza innego runtime tokio).
    pub fn new(channel: Arc<A>) -> Result<Self, UiError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .map_err(|e| UiError::Link(e.to_string()))?;
        Ok(Self { channel, runtime })
    }
}

impl<A: ApprovalChannel + ?Sized> BrokerLink for ChannelLink<A> {
    fn pending(&mut self) -> Result<Vec<ApprovalChallenge>, UiError> {
        Ok(self.channel.pending())
    }

    fn resolve(&mut self, d: UiDecision) -> Result<(), UiError> {
        self.runtime
            .block_on(self.channel.resolve(d.id, d.decision, d.proof))
            .map_err(|e| UiError::Link(e.to_string()))
    }
}

/// Łącze przez named pipe (protokół IPC Brokera, rola `BrokerUi`).
pub struct PipeLink {
    client: BlockingClient<Box<dyn PipeConnection>>,
}

/// Limit czekania na wolną instancję potoku (ms).
const CONNECT_TIMEOUT_MS: u32 = 5_000;

impl PipeLink {
    /// Łączy się według biletu: konto serwera musi być zgodne z `broker_user` biletu.
    pub fn connect(
        pipes: &dyn SecurePipePort,
        identity: &dyn ProcessIdentityPort,
        ticket: &UiLaunchTicket,
        my_pid: u32,
    ) -> Result<Self, UiError> {
        let link = |e: &dyn std::fmt::Display| UiError::Link(e.to_string());
        let conn = pipes
            .connect(&ticket.pipe, CONNECT_TIMEOUT_MS)
            .map_err(|e| link(&e))?;
        if let Some(expected) = &ticket.broker_user {
            let server = identity.identify(conn.peer_pid()).map_err(|e| link(&e))?;
            if server.user.as_str() != expected {
                return Err(UiError::Link(format!(
                    "serwer potoku działa na koncie {} zamiast {expected} — możliwe podstawienie",
                    server.user
                )));
            }
        }
        let hello = Hello {
            protocol: PROTOCOL_VERSION,
            credential: ticket.credential.clone(),
            pid: my_pid,
            sid: None,
            image: None,
        };
        let client = BlockingClient::connect(conn, &hello).map_err(|e| link(&e))?;
        Ok(Self { client })
    }

    fn call(&mut self, req: Request) -> Result<Response, UiError> {
        match self.client.call(req) {
            Ok(Response::Error(e)) => Err(UiError::Link(e.to_string())),
            Ok(r) => Ok(r),
            Err(e) => Err(UiError::Link(e.to_string())),
        }
    }
}

impl BrokerLink for PipeLink {
    fn pending(&mut self) -> Result<Vec<ApprovalChallenge>, UiError> {
        match self.call(Request::PendingApprovals)? {
            Response::Pending(list) => Ok(list),
            other => Err(UiError::Link(format!("nieoczekiwana odpowiedź: {other:?}"))),
        }
    }

    fn resolve(&mut self, d: UiDecision) -> Result<(), UiError> {
        let proof = d.proof_wire();
        let req = Request::Resolve {
            id: d.id,
            decision: d.decision,
            proof,
        };
        match self.call(req)? {
            Response::Ok => Ok(()),
            other => Err(UiError::Link(format!("nieoczekiwana odpowiedź: {other:?}"))),
        }
    }
}
