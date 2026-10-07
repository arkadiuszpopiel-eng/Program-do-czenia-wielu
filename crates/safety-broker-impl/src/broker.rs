//! Implementacje traitów kontraktu (`Broker`, `ApprovalChannel`) na silniku.

use async_trait::async_trait;
use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::AutonomyLevel;
use safety_broker_contract::{
    ActionRequest, ApprovalChallenge, ApprovalChannel, ApprovalDecision, ApprovalId,
    ApprovalStatus, AttenuateRequest, AutonomyChangeRequest, Broker, BrokerError, BrokerMetrics,
    CapToken, Capability, ChangeOrigin, Decision, Holder, KernelPolicy, PhysicalInputProof,
    PlanDecision, PlanRequest, SessionSecurity, TaintSource, TokenId,
};

use crate::engine::BrokerEngine;
use crate::state::PendStatus;

#[async_trait]
impl Broker for BrokerEngine {
    async fn decide(&self, action: ActionRequest) -> Result<Decision, BrokerError> {
        self.decide_sync(action)
    }

    fn verify(
        &self,
        token: &CapToken,
        needed: &Capability,
        presenter: &Holder,
    ) -> Result<(), BrokerError> {
        self.verify_sync(token, needed, presenter)
    }

    async fn attenuate(
        &self,
        parent: &CapToken,
        presenter: &Holder,
        request: AttenuateRequest,
    ) -> Result<CapToken, BrokerError> {
        self.attenuate_sync(parent, presenter, request)
    }

    async fn revoke(&self, id: TokenId) -> Result<usize, BrokerError> {
        self.revoke_sync(id)
    }

    async fn revoke_holder(&self, holder: &Holder) -> Result<usize, BrokerError> {
        Self::check_holder(holder)?;
        Ok(self.revoke_holder_sync(holder))
    }

    async fn report_untrusted_input(
        &self,
        session: &SessionId,
        source: TaintSource,
    ) -> Result<(), BrokerError> {
        let holder = Holder {
            session: session.clone(),
            agent: None,
            role: None,
        };
        Self::check_holder(&holder)?;
        let mut st = self.lock();
        self.taint(&mut st, &holder, source);
        Ok(())
    }

    fn session_security(&self, session: &SessionId) -> SessionSecurity {
        self.lock().session(session)
    }

    async fn submit_plan(&self, plan: PlanRequest) -> Result<PlanDecision, BrokerError> {
        self.submit_plan_sync(plan)
    }

    fn approval_status(
        &self,
        id: ApprovalId,
        requester: &Holder,
    ) -> Result<ApprovalStatus, BrokerError> {
        self.status_sync(id, requester)
    }

    async fn request_autonomy_change(
        &self,
        request: AutonomyChangeRequest,
    ) -> Result<Option<ApprovalId>, BrokerError> {
        self.autonomy_sync(request)
    }

    fn autonomy(&self, session: &SessionId, agent: Option<&AgentId>) -> AutonomyLevel {
        let now = self.now();
        self.lock().autonomy.effective(session, agent, now)
    }

    async fn request_policy_change(
        &self,
        policy: KernelPolicy,
        origin: ChangeOrigin,
    ) -> Result<ApprovalId, BrokerError> {
        self.policy_sync(policy, origin)
    }

    fn metrics(&self) -> BrokerMetrics {
        let now = self.now();
        let mut st = self.lock();
        Self::expire(&mut st, now);
        BrokerMetrics {
            approvals_last_hour: st.approvals_last_hour(now),
            active_tokens: u32::try_from(
                st.tokens.values().filter(|m| now < m.expires_at_ms).count(),
            )
            .unwrap_or(u32::MAX),
            pending_approvals: u32::try_from(
                st.approvals
                    .values()
                    .filter(|p| matches!(p.status, PendStatus::Pending))
                    .count(),
            )
            .unwrap_or(u32::MAX),
            kernel_blocks: st.kernel_blocks,
        }
    }
}

#[async_trait]
impl ApprovalChannel for BrokerEngine {
    fn pending(&self) -> Vec<ApprovalChallenge> {
        self.pending_sync()
    }

    async fn resolve(
        &self,
        id: ApprovalId,
        decision: ApprovalDecision,
        proof: PhysicalInputProof,
    ) -> Result<(), BrokerError> {
        self.resolve_sync(id, decision, proof)
    }
}
