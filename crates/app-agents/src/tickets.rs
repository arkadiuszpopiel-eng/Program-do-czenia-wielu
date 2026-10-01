//! `TicketLog` — przezroczysty dekorator Brokera dla narzędzi agentek: deleguje każdą metodę
//! bez zmian (decyzje podejmuje wyłącznie Broker) i zapamiętuje, czego dotyczyły prośby
//! o zatwierdzenie (narzędzie, zdolność, odwracalność, ryzyko, wyjaśnienie) — karta
//! „czeka na zatwierdzenie" w wątku potrzebuje tych faktów, a zdarzenie `agent.step.*` niesie
//! tylko numer prośby.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::{AutonomyLevel, Reversibility, RiskLevel};
use safety_broker_contract::{
    ActionRequest, ApprovalId, ApprovalStatus, AttenuateRequest, AutonomyChangeRequest, Broker,
    BrokerError, BrokerMetrics, CapToken, Capability, ChangeOrigin, Decision, Holder, KernelPolicy,
    PlanDecision, PlanRequest, SessionSecurity, TaintSource, TokenId,
};

/// Ile ostatnich próśb pamiętać.
const MAX_NOTES: usize = 256;

/// Fakty prośby o zatwierdzenie (do karty w UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketNote {
    /// Sesja proszącej agentki.
    pub session: SessionId,
    /// Narzędzie (`tools-fs.delete_permanent`).
    pub tool: String,
    /// Zdolność (np. `fs.write(c:\…)`).
    pub capability: String,
    /// Odwracalność z manifestu.
    pub reversible: Reversibility,
    /// Klasa ryzyka z Brokera.
    pub risk: RiskLevel,
    /// Wyjaśnienie Brokera (po polsku).
    pub explanation: String,
}

/// Dekorator Brokera zapamiętujący prośby o zatwierdzenie.
pub struct TicketLog {
    inner: Arc<dyn Broker>,
    notes: Mutex<BTreeMap<ApprovalId, TicketNote>>,
}

impl TicketLog {
    /// Dekorator nad Brokerem.
    pub fn new(inner: Arc<dyn Broker>) -> Self {
        Self {
            inner,
            notes: Mutex::new(BTreeMap::new()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<ApprovalId, TicketNote>> {
        self.notes.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Fakty prośby (tylko dla sesji, która prosiła).
    pub fn note(&self, id: ApprovalId, session: &SessionId) -> Option<TicketNote> {
        self.lock()
            .get(&id)
            .filter(|n| &n.session == session)
            .cloned()
    }

    /// Stan prośby po rozstrzygnięciu (`Approved` bez tokenu — token już odebrany).
    pub fn resolved(&self, id: ApprovalId, holder: &Holder) -> Option<ApprovalStatus> {
        self.inner.approval_status(id, holder).ok()
    }
}

#[async_trait]
impl Broker for TicketLog {
    async fn decide(&self, action: ActionRequest) -> Result<Decision, BrokerError> {
        let session = action.holder.session.clone();
        let tool = action.facts.tool.clone();
        let reversible = action.facts.reversible;
        let capability = action.capability.to_string();
        let decision = self.inner.decide(action).await?;
        if let Decision::NeedsApproval(ticket) = &decision {
            let mut notes = self.lock();
            notes.insert(
                ticket.id,
                TicketNote {
                    session,
                    tool,
                    capability,
                    reversible,
                    risk: ticket.risk,
                    explanation: ticket.explanation.clone(),
                },
            );
            while notes.len() > MAX_NOTES {
                notes.pop_first();
            }
        }
        Ok(decision)
    }

    fn verify(
        &self,
        token: &CapToken,
        needed: &Capability,
        presenter: &Holder,
    ) -> Result<(), BrokerError> {
        self.inner.verify(token, needed, presenter)
    }

    async fn attenuate(
        &self,
        parent: &CapToken,
        presenter: &Holder,
        request: AttenuateRequest,
    ) -> Result<CapToken, BrokerError> {
        self.inner.attenuate(parent, presenter, request).await
    }

    async fn revoke(&self, id: TokenId) -> Result<usize, BrokerError> {
        self.inner.revoke(id).await
    }

    async fn revoke_holder(&self, holder: &Holder) -> Result<usize, BrokerError> {
        self.inner.revoke_holder(holder).await
    }

    async fn report_untrusted_input(
        &self,
        session: &SessionId,
        source: TaintSource,
    ) -> Result<(), BrokerError> {
        self.inner.report_untrusted_input(session, source).await
    }

    fn session_security(&self, session: &SessionId) -> SessionSecurity {
        self.inner.session_security(session)
    }

    async fn submit_plan(&self, plan: PlanRequest) -> Result<PlanDecision, BrokerError> {
        self.inner.submit_plan(plan).await
    }

    fn approval_status(
        &self,
        id: ApprovalId,
        requester: &Holder,
    ) -> Result<ApprovalStatus, BrokerError> {
        self.inner.approval_status(id, requester)
    }

    async fn request_autonomy_change(
        &self,
        request: AutonomyChangeRequest,
    ) -> Result<Option<ApprovalId>, BrokerError> {
        self.inner.request_autonomy_change(request).await
    }

    fn autonomy(&self, session: &SessionId, agent: Option<&AgentId>) -> AutonomyLevel {
        self.inner.autonomy(session, agent)
    }

    async fn request_policy_change(
        &self,
        policy: KernelPolicy,
        origin: ChangeOrigin,
    ) -> Result<ApprovalId, BrokerError> {
        self.inner.request_policy_change(policy, origin).await
    }

    fn metrics(&self) -> BrokerMetrics {
        self.inner.metrics()
    }
}
