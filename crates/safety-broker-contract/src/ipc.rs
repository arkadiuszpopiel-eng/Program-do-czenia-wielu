//! Protokół IPC Broker ↔ klienci (wersjonowany, ramki JSON z prefiksem długości).
//!
//! Transport produkcyjny: named pipe z ACL na SID (część 2, przez `platform-windows`);
//! **brak nasłuchu TCP**. Uwierzytelnienie: poświadczenie klienta wydane przez Brokera przy
//! starcie procesu (rola + TTL + MAC); w przyszłości dodatkowo SID i Authenticode procesu
//! po drugiej stronie potoku. Uprawnienia operacji wynikają z roli ([`Request::permitted`]);
//! źródło zmian (`ChangeOrigin`) ustala serwer z roli, nie z treści żądania.

use core_bus_contract::{AgentId, Event, SessionId};
use risk_classifier_contract::AutonomyLevel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use watchdog_contract::{KillReason, KillReport};

use crate::action::{
    ActionRequest, AttenuateRequest, BrokerError, BrokerMetrics, Decision, PlanDecision,
    PlanRequest, SessionSecurity, TaintSource,
};
use crate::approval::{ApprovalChallenge, ApprovalDecision, ApprovalId, ApprovalStatus};
use crate::autonomy::AutonomyTarget;
use crate::capability::Capability;
use crate::policy::KernelPolicy;
use crate::proof::{InputSource, Nonce};
use crate::token::{CapToken, Holder, TokenId};

/// Wersja protokołu.
pub const PROTOCOL_VERSION: u16 = 1;
/// Maksymalny rozmiar ramki (bajty treści).
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Rola klienta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClientRole {
    /// Jądro Alfy (przekaźnik UI i głosu).
    Core,
    /// Proces agentki / narzędzia.
    Agent,
    /// Okno zatwierdzeń.
    BrokerUi,
    /// Watchdog.
    Watchdog,
}

/// Poświadczenie klienta (wydaje Broker przy uruchomieniu procesu; MAC kluczem Brokera).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClientCredential {
    /// Identyfikator klienta.
    pub client_id: String,
    /// Rola.
    pub role: ClientRole,
    /// Wygasa (ms).
    pub expires_at_ms: u64,
    /// MAC (hex) nad `client_id`, rolą, terminem i uruchomieniem Brokera.
    pub mac: String,
}

/// Powitanie klienta (pierwsza ramka połączenia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Hello {
    /// Wersja protokołu klienta.
    pub protocol: u16,
    /// Poświadczenie.
    pub credential: ClientCredential,
    /// PID klienta (z `GetNamedPipeClientProcessId` po stronie serwera — część 2).
    pub pid: u32,
    /// SID konta klienta (część 2: weryfikacja z tokenu procesu).
    pub sid: Option<String>,
    /// Ścieżka obrazu procesu (część 2: weryfikacja Authenticode).
    pub image: Option<String>,
}

/// Odpowiedź na powitanie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "hello", rename_all = "snake_case")]
pub enum HelloReply {
    /// Przyjęto.
    Welcome {
        /// Wersja protokołu serwera.
        protocol: u16,
    },
    /// Odrzucono (połączenie zostanie zamknięte).
    Rejected {
        /// Powód.
        reason: String,
    },
}

/// Dowód fizycznego wejścia w postaci przewodowej — serwer zamienia go na
/// `PhysicalInputProof` wyłącznie dla połączeń z rolą `BrokerUi`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofWire {
    /// Prośba.
    pub approval: ApprovalId,
    /// Nonce wyzwania.
    pub nonce: Nonce,
    /// Źródło wejścia.
    pub source: InputSource,
    /// Flaga wstrzyknięcia.
    pub injected: bool,
    /// Chwila wejścia (ms).
    pub at_ms: u64,
}

/// Źródło żądania zmiany deklarowane przez jądro (agentka nie może go deklarować).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserChannel {
    /// Ustawienia / pasek sesji.
    UserInterface,
    /// Polecenie głosowe właściciela.
    UserVoice,
}

/// Żądanie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", content = "args", rename_all = "snake_case")]
pub enum Request {
    /// `Broker::decide`.
    Decide(ActionRequest),
    /// `Broker::verify`.
    Verify {
        /// Token.
        token: CapToken,
        /// Potrzebna zdolność.
        needed: Capability,
        /// Okaziciel.
        presenter: Holder,
    },
    /// `Broker::attenuate`.
    Attenuate {
        /// Rodzic.
        parent: CapToken,
        /// Okaziciel rodzica.
        presenter: Holder,
        /// Żądanie.
        request: AttenuateRequest,
    },
    /// `Broker::revoke`.
    Revoke {
        /// Token.
        id: TokenId,
    },
    /// `Broker::revoke_holder`.
    RevokeHolder {
        /// Podmiot.
        holder: Holder,
    },
    /// `Broker::report_untrusted_input`.
    ReportUntrusted {
        /// Sesja.
        session: SessionId,
        /// Źródło.
        source: TaintSource,
    },
    /// `Broker::session_security`.
    SessionSecurity {
        /// Sesja.
        session: SessionId,
    },
    /// `Broker::submit_plan`.
    SubmitPlan(PlanRequest),
    /// `Broker::approval_status`.
    ApprovalStatus {
        /// Prośba.
        id: ApprovalId,
        /// Proszący.
        requester: Holder,
    },
    /// `Broker::request_autonomy_change` (źródło ustala serwer).
    RequestAutonomy {
        /// Cel.
        target: AutonomyTarget,
        /// Poziom.
        level: AutonomyLevel,
        /// Do kiedy.
        until_ms: Option<u64>,
        /// Kanał deklarowany przez jądro (ignorowany dla roli `Agent`).
        via: UserChannel,
    },
    /// `Broker::autonomy`.
    Autonomy {
        /// Sesja.
        session: SessionId,
        /// Agentka.
        agent: Option<AgentId>,
    },
    /// `Broker::request_policy_change` (źródło ustala serwer).
    RequestPolicy(Box<KernelPolicy>),
    /// `Broker::metrics`.
    Metrics,
    /// `ApprovalChannel::pending` — tylko Broker-UI.
    PendingApprovals,
    /// `ApprovalChannel::resolve` — tylko Broker-UI.
    Resolve {
        /// Prośba.
        id: ApprovalId,
        /// Decyzja.
        decision: ApprovalDecision,
        /// Dowód.
        proof: ProofWire,
    },
    /// Kill-switch (jądro, Broker-UI, watchdog).
    KillAll {
        /// Powód.
        reason: KillReason,
    },
    /// Zapis zdarzenia do Audytu (Broker jedynym writerem; jądro i watchdog przekazują).
    AuditAppend(Box<Event>),
}

impl Request {
    /// Czy rola może wykonać to żądanie.
    pub fn permitted(&self, role: ClientRole) -> bool {
        use ClientRole::{Agent, BrokerUi, Core, Watchdog};
        match self {
            Self::Decide(_)
            | Self::Verify { .. }
            | Self::Attenuate { .. }
            | Self::Revoke { .. }
            | Self::ReportUntrusted { .. }
            | Self::SubmitPlan(_)
            | Self::ApprovalStatus { .. }
            | Self::RequestAutonomy { .. }
            | Self::RequestPolicy(_) => matches!(role, Core | Agent),
            Self::SessionSecurity { .. } | Self::Autonomy { .. } => {
                matches!(role, Core | Agent | BrokerUi)
            }
            Self::RevokeHolder { .. } => role == Core,
            Self::Metrics => matches!(role, Core | BrokerUi | Watchdog),
            Self::PendingApprovals | Self::Resolve { .. } => role == BrokerUi,
            Self::KillAll { .. } => matches!(role, Core | BrokerUi | Watchdog),
            Self::AuditAppend(_) => matches!(role, Core | Watchdog),
        }
    }
}

/// Odpowiedź.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "result", content = "value", rename_all = "snake_case")]
pub enum Response {
    /// Bez wartości.
    Ok,
    /// Decyzja.
    Decision(Decision),
    /// Token.
    Token(CapToken),
    /// Liczba.
    Count(u64),
    /// Stan sesji.
    Security(SessionSecurity),
    /// Wynik planu.
    Plan(PlanDecision),
    /// Stan prośby.
    Status(ApprovalStatus),
    /// Prośba (albo brak — zmiana zastosowana od razu).
    Approval(Option<ApprovalId>),
    /// Poziom.
    Level(AutonomyLevel),
    /// Metryki.
    Metrics(BrokerMetrics),
    /// Oczekujące wyzwania.
    Pending(Vec<ApprovalChallenge>),
    /// Raport kill-switcha.
    Killed(KillReport),
    /// Rekord Audytu (seq, hash).
    Audited {
        /// Numer.
        seq: u64,
        /// Hash.
        hash: String,
    },
    /// Błąd.
    Error(BrokerError),
}

/// Koperta ramki: wersja protokołu + numer żądania (odpowiedź niesie ten sam).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Envelope<T> {
    /// Wersja protokołu.
    pub v: u16,
    /// Numer żądania.
    pub id: u64,
    /// Treść.
    pub body: T,
}

/// Błąd ramkowania.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    /// Ramka większa niż [`MAX_FRAME_BYTES`].
    #[error("ramka za duża: {0} B")]
    TooLarge(usize),
    /// Niepoprawny JSON / niezgodny schemat.
    #[error("niepoprawna ramka: {0}")]
    Malformed(String),
    /// Niezgodna wersja protokołu.
    #[error("niezgodna wersja protokołu: {0}")]
    Version(u16),
}

/// Koduje wiadomość jako ramkę: 4 bajty długości (LE) + JSON.
pub fn encode_frame<T: Serialize>(msg: &T) -> Result<Vec<u8>, FrameError> {
    let body = serde_json::to_vec(msg).map_err(|e| FrameError::Malformed(e.to_string()))?;
    if body.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(body.len()));
    }
    let len = u32::try_from(body.len()).map_err(|_| FrameError::TooLarge(body.len()))?;
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Długość treści z nagłówka ramki (odrzuca ramki ponad limit przed alokacją).
pub fn frame_len(header: [u8; 4]) -> Result<usize, FrameError> {
    let len = usize::try_from(u32::from_le_bytes(header)).map_err(|_| FrameError::TooLarge(0))?;
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len));
    }
    Ok(len)
}

/// Dekoduje treść ramki.
pub fn decode_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, FrameError> {
    serde_json::from_slice(body).map_err(|e| FrameError::Malformed(e.to_string()))
}
