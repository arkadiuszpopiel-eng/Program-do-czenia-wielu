//! Serwer i klient protokołu IPC Brokera na dowolnym strumieniu bajtów (`AsyncRead +
//! AsyncWrite`). Produkcyjnie strumieniem będzie named pipe z ACL na SID (część 2, przez
//! `platform-windows`); w testach — `tokio::io::duplex` ([`in_memory_pair`]). Ten moduł
//! nie otwiera żadnych gniazd sieciowych.

use std::sync::Arc;

use safety_broker_contract::ipc::{
    ClientRole, Envelope, FrameError, Hello, HelloReply, MAX_FRAME_BYTES, PROTOCOL_VERSION,
    Request, Response, UserChannel, decode_body, encode_frame, frame_len,
};
use safety_broker_contract::{ApprovalChannel, Broker, BrokerError, ChangeOrigin, broker_ui_only};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};
use watchdog_contract::KillSwitch;

use crate::engine::BrokerEngine;

/// Błąd połączenia IPC.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IpcError {
    /// Błąd ramki.
    #[error(transparent)]
    Frame(#[from] FrameError),
    /// Błąd strumienia.
    #[error("strumień IPC: {0}")]
    Io(String),
    /// Serwer odrzucił powitanie.
    #[error("połączenie odrzucone: {0}")]
    Rejected(String),
    /// Druga strona zamknęła połączenie.
    #[error("połączenie zamknięte")]
    Closed,
}

/// Czyta jedną ramkę; `Ok(None)` przy czystym końcu strumienia.
pub async fn read_frame<R, T>(r: &mut R) -> Result<Option<T>, IpcError>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let mut header = [0u8; 4];
    match r.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(IpcError::Io(e.to_string())),
    }
    let len = frame_len(header)?;
    let mut body = vec![0u8; len.min(MAX_FRAME_BYTES)];
    r.read_exact(&mut body)
        .await
        .map_err(|e| IpcError::Io(e.to_string()))?;
    Ok(Some(decode_body(&body)?))
}

/// Zapisuje jedną ramkę.
pub async fn write_frame<W, T>(w: &mut W, msg: &T) -> Result<(), IpcError>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let frame = encode_frame(msg)?;
    w.write_all(&frame)
        .await
        .map_err(|e| IpcError::Io(e.to_string()))?;
    w.flush().await.map_err(|e| IpcError::Io(e.to_string()))
}

/// Para połączonych strumieni w pamięci (testy, transport wewnątrzprocesowy).
pub fn in_memory_pair() -> (DuplexStream, DuplexStream) {
    tokio::io::duplex(64 * 1024)
}

/// Serwer protokołu: powitanie z poświadczeniem, potem żądania autoryzowane rolą.
pub struct BrokerServer {
    engine: Arc<BrokerEngine>,
}

impl BrokerServer {
    /// Serwer nad silnikiem.
    pub fn new(engine: Arc<BrokerEngine>) -> Self {
        Self { engine }
    }

    /// Obsługuje jedno połączenie do końca strumienia (powitanie: poświadczenie z MAC).
    pub async fn serve<S>(&self, stream: S) -> Result<(), IpcError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let engine = self.engine.clone();
        self.serve_with(
            stream,
            move |hello: &Hello| {
                engine
                    .verify_client_credential(&hello.credential)
                    .map_err(|e| e.to_string())
            },
            |_| {},
        )
        .await
    }

    /// Jak [`Self::serve`], ale o przyjęciu powitania decyduje `authorize` — usługa sprawdza
    /// w nim tożsamość procesu po drugiej stronie potoku (SID, integralność, obraz) i poświadczenie.
    /// `on_reject` dostaje powód odmowy **zanim** klient zobaczy odpowiedź (najpierw Audyt,
    /// potem odpowiedź — klient nie wyprzedzi zapisu).
    pub async fn serve_with<S, F, R>(
        &self,
        mut stream: S,
        authorize: F,
        on_reject: R,
    ) -> Result<(), IpcError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
        F: FnOnce(&Hello) -> Result<(), String>,
        R: FnOnce(&str),
    {
        let Some(hello) = read_frame::<_, Hello>(&mut stream).await? else {
            return Err(IpcError::Closed);
        };
        let verdict = if hello.protocol != PROTOCOL_VERSION {
            Err(format!(
                "wersja protokołu {} ≠ {PROTOCOL_VERSION}",
                hello.protocol
            ))
        } else {
            authorize(&hello)
        };
        if let Err(reason) = verdict {
            on_reject(&reason);
            write_frame(
                &mut stream,
                &HelloReply::Rejected {
                    reason: reason.clone(),
                },
            )
            .await?;
            return Err(IpcError::Rejected(reason));
        }
        write_frame(
            &mut stream,
            &HelloReply::Welcome {
                protocol: PROTOCOL_VERSION,
            },
        )
        .await?;
        let (role, client) = (hello.credential.role, hello.credential.client_id);
        while let Some(env) = read_frame::<_, Envelope<Request>>(&mut stream).await? {
            let body = if env.v != PROTOCOL_VERSION {
                Response::Error(BrokerError::InvalidRequest(
                    "niezgodna wersja protokołu".into(),
                ))
            } else if !env.body.permitted(role) {
                Response::Error(BrokerError::Unauthorized(format!("rola {role:?}")))
            } else {
                self.handle(role, &client, env.body).await
            };
            let reply = Envelope {
                v: PROTOCOL_VERSION,
                id: env.id,
                body,
            };
            write_frame(&mut stream, &reply).await?;
        }
        Ok(())
    }

    fn origin(role: ClientRole, client: &str, via: UserChannel) -> ChangeOrigin {
        match (role, via) {
            (ClientRole::Core, UserChannel::UserInterface) => ChangeOrigin::UserInterface,
            (ClientRole::Core, UserChannel::UserVoice) => ChangeOrigin::UserVoice,
            // Źródło deklarowane przez inne role jest ignorowane — agentka zawsze jest agentką.
            _ => ChangeOrigin::Agent(client.to_owned()),
        }
    }

    /// Wykonuje żądanie już autoryzowane rolą.
    pub async fn handle(&self, role: ClientRole, client: &str, req: Request) -> Response {
        let e = &self.engine;
        let res = match req {
            Request::Decide(a) => e.decide(a).await.map(Response::Decision),
            Request::Verify {
                token,
                needed,
                presenter,
            } => e.verify(&token, &needed, &presenter).map(|()| Response::Ok),
            Request::Attenuate {
                parent,
                presenter,
                request,
            } => e
                .attenuate(&parent, &presenter, request)
                .await
                .map(Response::Token),
            Request::Revoke { id } => e.revoke(id).await.map(|n| Response::Count(n as u64)),
            Request::RevokeHolder { holder } => e
                .revoke_holder(&holder)
                .await
                .map(|n| Response::Count(n as u64)),
            Request::ReportUntrusted { session, source } => e
                .report_untrusted_input(&session, source)
                .await
                .map(|()| Response::Ok),
            Request::SessionSecurity { session } => {
                Ok(Response::Security(e.session_security(&session)))
            }
            Request::SubmitPlan(p) => e.submit_plan(p).await.map(Response::Plan),
            Request::ApprovalStatus { id, requester } => {
                e.approval_status(id, &requester).map(Response::Status)
            }
            Request::RequestAutonomy {
                target,
                level,
                until_ms,
                via,
            } => {
                let origin = Self::origin(role, client, via);
                let req = safety_broker_contract::AutonomyChangeRequest {
                    target,
                    level,
                    until_ms,
                    origin,
                };
                e.request_autonomy_change(req).await.map(Response::Approval)
            }
            Request::Autonomy { session, agent } => {
                Ok(Response::Level(e.autonomy(&session, agent.as_ref())))
            }
            Request::RequestPolicy(policy) => {
                let origin = Self::origin(role, client, UserChannel::UserInterface);
                e.request_policy_change(*policy, origin)
                    .await
                    .map(|id| Response::Approval(Some(id)))
            }
            Request::Metrics => Ok(Response::Metrics(e.metrics())),
            Request::PendingApprovals => Ok(Response::Pending(e.pending())),
            Request::Resolve {
                id,
                decision,
                proof,
            } => {
                // Dowód powstaje wyłącznie tu i wyłącznie dla roli Broker-UI (sprawdzone wyżej).
                let proof = broker_ui_only::physical_input_proof(
                    proof.approval,
                    proof.nonce,
                    proof.source,
                    proof.injected,
                    proof.at_ms,
                );
                e.resolve(id, decision, proof).await.map(|()| Response::Ok)
            }
            Request::KillAll { reason } => Ok(Response::Killed(e.kill_all(reason).await)),
            Request::AuditAppend(event) => {
                e.append_external(*event, client)
                    .map(|r| Response::Audited {
                        seq: r.seq,
                        hash: r.hash,
                    })
            }
        };
        res.unwrap_or_else(Response::Error)
    }
}

/// Klient protokołu.
pub struct BrokerClient<S> {
    stream: S,
    next_id: u64,
}

impl<S: AsyncRead + AsyncWrite + Unpin> BrokerClient<S> {
    /// Łączy się: wysyła powitanie i czeka na przyjęcie.
    pub async fn connect(mut stream: S, hello: Hello) -> Result<Self, IpcError> {
        write_frame(&mut stream, &hello).await?;
        match read_frame::<_, HelloReply>(&mut stream).await? {
            Some(HelloReply::Welcome { .. }) => Ok(Self { stream, next_id: 0 }),
            Some(HelloReply::Rejected { reason }) => Err(IpcError::Rejected(reason)),
            None => Err(IpcError::Closed),
        }
    }

    /// Wysyła żądanie i czeka na odpowiedź o tym samym numerze.
    pub async fn call(&mut self, body: Request) -> Result<Response, IpcError> {
        self.next_id += 1;
        let id = self.next_id;
        write_frame(
            &mut self.stream,
            &Envelope {
                v: PROTOCOL_VERSION,
                id,
                body,
            },
        )
        .await?;
        match read_frame::<_, Envelope<Response>>(&mut self.stream).await? {
            Some(env) if env.id == id => Ok(env.body),
            Some(_) => Err(IpcError::Io("odpowiedź na inne żądanie".into())),
            None => Err(IpcError::Closed),
        }
    }
}
