//! Traity Brokera: [`Broker`] (strona agentek i jądra), [`ApprovalChannel`] (wyłącznie
//! Broker-UI), [`AnchorStore`] (kotwica głowy łańcucha Audytu) oraz nazwy zdarzeń Audytu.

use async_trait::async_trait;
use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::AutonomyLevel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::action::{
    ActionRequest, AttenuateRequest, AutonomyChangeRequest, BrokerError, BrokerMetrics,
    ChangeOrigin, Decision, PlanDecision, PlanRequest, SessionSecurity, TaintSource,
};
use crate::approval::{ApprovalChallenge, ApprovalDecision, ApprovalId, ApprovalStatus};
use crate::capability::Capability;
use crate::policy::KernelPolicy;
use crate::proof::PhysicalInputProof;
use crate::token::{CapToken, Holder, TokenId};

/// Audyt: wydano token.
pub const EVENT_TOKEN_ISSUED: &str = "broker.token.issued";
/// Audyt: unieważniono tokeny.
pub const EVENT_TOKEN_REVOKED: &str = "broker.token.revoked";
/// Audyt: odrzucono token przy weryfikacji (MAC, uruchomienie, zakres).
pub const EVENT_TOKEN_DENIED: &str = "broker.token.denied";
/// Audyt: prośba o zatwierdzenie.
pub const EVENT_APPROVAL_REQUESTED: &str = "broker.approval.requested";
/// Audyt: decyzja właściciela.
pub const EVENT_APPROVAL_DECIDED: &str = "broker.approval.decided";
/// Audyt: zmiana poziomu autonomii.
pub const EVENT_AUTONOMY_CHANGED: &str = "broker.autonomy.changed";
/// Audyt: twarda blokada Jądra.
pub const EVENT_KERNEL_BLOCK: &str = "broker.kernel_block";
/// Audyt: kill-switch.
pub const EVENT_KILL_SWITCH: &str = "broker.kill_switch";
/// Audyt: zmiana polityk Jądra.
pub const EVENT_POLICY_CHANGED: &str = "broker.policy.changed";
/// Audyt: sesja oznaczona `tainted`.
pub const EVENT_SESSION_TAINTED: &str = "broker.session.tainted";
/// Audyt: rotacja klucza MAC.
pub const EVENT_KEY_ROTATED: &str = "broker.key.rotated";
/// Audyt: początek nowego łańcucha (przejęcie strumienia od `pre-broker`).
pub const EVENT_CHAIN_STARTED: &str = "broker.audit.chain_started";

/// Kotwica głowy łańcucha Audytu (przechowywana poza plikiem łańcucha).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChainAnchor {
    /// Identyfikator łańcucha.
    pub chain_id: String,
    /// Liczba rekordów.
    pub records: u64,
    /// Hash ostatniego rekordu (hex).
    pub head: String,
}

/// Magazyn kotwicy (plik pod ACL konta usługi; w przyszłości TPM — THREAT_MODEL §11).
pub trait AnchorStore: Send + Sync {
    /// Ostatnio zapisana kotwica.
    fn load(&self) -> Result<Option<ChainAnchor>, String>;
    /// Zapisuje kotwicę atomowo.
    fn store(&self, anchor: &ChainAnchor) -> Result<(), String>;
}

/// Broker — strona agentek i jądra (przez IPC). Nie ma metod zatwierdzania: te są wyłącznie
/// w [`ApprovalChannel`] i wymagają [`PhysicalInputProof`].
#[async_trait]
pub trait Broker: Send + Sync {
    /// Decyzja o akcji: token, prośba o zatwierdzenie albo odmowa (każda zapisana w Audycie).
    async fn decide(&self, action: ActionRequest) -> Result<Decision, BrokerError>;

    /// Weryfikacja tokenu dla konkretnego użycia (`needed` ⊆ zakres, podmiot, MAC, TTL,
    /// uruchomienie, unieważnienia, reguły Jądra dla konkretnego celu).
    fn verify(
        &self,
        token: &CapToken,
        needed: &Capability,
        presenter: &Holder,
    ) -> Result<(), BrokerError>;

    /// Token potomny: zakres ⊆ rodzic, TTL ≤ pozostały czas rodzica, ta sama sesja i agentka.
    async fn attenuate(
        &self,
        parent: &CapToken,
        presenter: &Holder,
        request: AttenuateRequest,
    ) -> Result<CapToken, BrokerError>;

    /// Unieważnia token i jego potomków; zwraca liczbę unieważnionych.
    async fn revoke(&self, id: TokenId) -> Result<usize, BrokerError>;

    /// Unieważnia wszystkie tokeny podmiotu (zmiana obsady = nowe tokeny).
    async fn revoke_holder(&self, holder: &Holder) -> Result<usize, BrokerError>;

    /// Zgłoszenie niezaufanej treści w sesji — sesja staje się `tainted` (monotonicznie).
    async fn report_untrusted_input(
        &self,
        session: &SessionId,
        source: TaintSource,
    ) -> Result<(), BrokerError>;

    /// Stan bezpieczeństwa sesji.
    fn session_security(&self, session: &SessionId) -> SessionSecurity;

    /// „Plan do zatwierdzenia”: jedno zatwierdzenie dla wielu kroków z zakresem.
    async fn submit_plan(&self, plan: PlanRequest) -> Result<PlanDecision, BrokerError>;

    /// Stan prośby — tylko dla podmiotu, który prosił; token zatwierdzonej akcji wydawany raz.
    fn approval_status(
        &self,
        id: ApprovalId,
        requester: &Holder,
    ) -> Result<ApprovalStatus, BrokerError>;

    /// Zmiana poziomu autonomii. Obniżenie działa od razu (`Ok(None)`); podniesienie tworzy
    /// prośbę do Broker-UI (`Ok(Some(id))`); podniesienie przez agentkę = `KernelBlock`.
    async fn request_autonomy_change(
        &self,
        request: AutonomyChangeRequest,
    ) -> Result<Option<ApprovalId>, BrokerError>;

    /// Poziom obowiązujący agentkę w sesji.
    fn autonomy(&self, session: &SessionId, agent: Option<&AgentId>) -> AutonomyLevel;

    /// Zmiana polityk Jądra — zawsze przez prośbę do Broker-UI; od agentki = `KernelBlock`.
    async fn request_policy_change(
        &self,
        policy: KernelPolicy,
        origin: ChangeOrigin,
    ) -> Result<ApprovalId, BrokerError>;

    /// Metryki (pytania/godz. itp.).
    fn metrics(&self) -> BrokerMetrics;
}

/// Kanał zatwierdzeń — WYŁĄCZNIE Broker-UI (w IPC tylko rola `BrokerUi`).
#[async_trait]
pub trait ApprovalChannel: Send + Sync {
    /// Oczekujące wyzwania (karty + nonce).
    fn pending(&self) -> Vec<ApprovalChallenge>;

    /// Rozstrzygnięcie prośby dowodem fizycznego wejścia (jednorazowy nonce, świeżość,
    /// wejście niewstrzyknięte, opcjonalnie Windows Hello).
    async fn resolve(
        &self,
        id: ApprovalId,
        decision: ApprovalDecision,
        proof: PhysicalInputProof,
    ) -> Result<(), BrokerError>;
}
