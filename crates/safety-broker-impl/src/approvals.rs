//! Prośby o zatwierdzenie: otwarcie (wyzwanie z nonce), stan dla proszącego, rozstrzygnięcie
//! dowodem fizycznego wejścia (wyłącznie kanał Broker-UI).

use risk_classifier_contract::{CommandOrigin, Reversibility, RiskLevel, RuleId};
use safety_broker_contract::{
    ApprovalChallenge, ApprovalDecision, ApprovalId, ApprovalRequest, ApprovalStatus,
    ApprovalSubject, ApprovalTicket, BrokerError, EVENT_APPROVAL_DECIDED, EVENT_APPROVAL_REQUESTED,
    Holder, InputSource, PhysicalInputProof,
};
use serde_json::json;

use crate::engine::BrokerEngine;
use crate::keys::ct_eq;
use crate::state::{PendStatus, Pending, PendingAction, State};

/// Metadane karty zatwierdzenia.
pub(crate) struct Meta {
    pub holder: Holder,
    pub risk: RiskLevel,
    pub reversible: Reversibility,
    pub origin: CommandOrigin,
    pub tainted: bool,
    pub non_voice: bool,
    pub grantable: bool,
    pub hello_required: bool,
    pub rules: Vec<RuleId>,
    pub explanation: String,
}

fn same_requester(a: &Holder, b: &Holder) -> bool {
    a.session == b.session && a.agent == b.agent
}

impl BrokerEngine {
    /// Otwiera prośbę: losowy nonce, termin, zapis do Audytu (błąd → prośba nie powstaje).
    pub(crate) fn open_approval(
        &self,
        st: &mut State,
        subject: ApprovalSubject,
        meta: Meta,
        action: PendingAction,
        now: u64,
    ) -> Result<ApprovalTicket, BrokerError> {
        let nonce = st.keys.nonce().map_err(BrokerError::InvalidRequest)?;
        let id = ApprovalId(st.next_approval + 1);
        let request = ApprovalRequest {
            id,
            holder: meta.holder,
            subject,
            risk: meta.risk,
            reversible: meta.reversible,
            origin: meta.origin,
            tainted: meta.tainted,
            non_voice: meta.non_voice,
            grantable: meta.grantable,
            hello_required: meta.hello_required,
            rules: meta.rules,
            explanation: meta.explanation,
            created_at_ms: now,
            expires_at_ms: now.saturating_add(st.policy().approval_ttl_ms),
        };
        let payload = json!({
            "approval": id, "subject": request.subject, "risk": request.risk,
            "rules": request.rules, "non_voice": request.non_voice, "tainted": request.tainted,
        });
        self.audit(EVENT_APPROVAL_REQUESTED, Some(&request.holder), payload)?;
        st.next_approval += 1;
        st.approval_times.push_back(now);
        let ticket = ApprovalTicket {
            id,
            risk: request.risk,
            rules: request.rules.clone(),
            non_voice: request.non_voice,
            explanation: request.explanation.clone(),
        };
        st.approvals.insert(
            id,
            Pending {
                request,
                nonce,
                action: Some(action),
                status: PendStatus::Pending,
            },
        );
        Ok(ticket)
    }

    /// Przeterminowane prośby → `Expired`.
    pub(crate) fn expire(st: &mut State, now: u64) {
        for p in st.approvals.values_mut() {
            if matches!(p.status, PendStatus::Pending) && now >= p.request.expires_at_ms {
                p.status = PendStatus::Expired;
            }
        }
    }

    /// `ApprovalChannel::pending`.
    pub(crate) fn pending_sync(&self) -> Vec<ApprovalChallenge> {
        let now = self.now();
        let mut st = self.lock();
        Self::expire(&mut st, now);
        st.approvals
            .values()
            .filter(|p| matches!(p.status, PendStatus::Pending))
            .map(|p| ApprovalChallenge {
                request: p.request.clone(),
                nonce: p.nonce,
            })
            .collect()
    }

    /// `Broker::approval_status` — tylko proszący; token wydawany raz.
    pub(crate) fn status_sync(
        &self,
        id: ApprovalId,
        requester: &Holder,
    ) -> Result<ApprovalStatus, BrokerError> {
        let now = self.now();
        let mut st = self.lock();
        Self::expire(&mut st, now);
        let p = st
            .approvals
            .get_mut(&id)
            .ok_or(BrokerError::UnknownApproval(id))?;
        if !same_requester(&p.request.holder, requester) {
            return Err(BrokerError::Unauthorized("prośba innego podmiotu".into()));
        }
        Ok(match &mut p.status {
            PendStatus::Pending => ApprovalStatus::Pending,
            PendStatus::Approved(token) => ApprovalStatus::Approved {
                token: token.take(),
            },
            PendStatus::Denied => ApprovalStatus::Denied,
            PendStatus::Expired => ApprovalStatus::Expired,
        })
    }

    fn check_proof(p: &Pending, proof: &PhysicalInputProof, now: u64) -> Result<(), String> {
        if proof.approval() != p.request.id {
            return Err("dowód dotyczy innej prośby".into());
        }
        if !ct_eq(&proof.nonce().0, &p.nonce.0) {
            return Err("niezgodny nonce wyzwania".into());
        }
        if proof.injected() {
            return Err("wejście wstrzyknięte (SendInput) — wymagane fizyczne".into());
        }
        if proof.at_ms() < p.request.created_at_ms || proof.at_ms() > now {
            return Err("dowód spoza okna prośby".into());
        }
        if p.request.hello_required && proof.source() != InputSource::WindowsHello {
            return Err("wymagane Windows Hello".into());
        }
        Ok(())
    }

    fn check_decision_shape(p: &Pending, decision: &ApprovalDecision) -> Result<(), BrokerError> {
        let ApprovalDecision::AllowInScope { scope, .. } = decision else {
            return Ok(());
        };
        let Some(PendingAction::Action(req)) = &p.action else {
            return Err(BrokerError::InvalidRequest(
                "„zawsze zezwalaj” dotyczy tylko pojedynczej akcji".into(),
            ));
        };
        if !p.request.grantable {
            return Err(BrokerError::InvalidRequest(
                "ta prośba nie może być pokryta „zawsze zezwalaj” (reguła każdego poziomu)".into(),
            ));
        }
        if scope.family() != req.capability.family() || !req.capability.is_subset_of(scope) {
            return Err(BrokerError::InvalidRequest(
                "zakres musi obejmować prośbę i należeć do tej samej rodziny".into(),
            ));
        }
        Ok(())
    }

    /// `ApprovalChannel::resolve`.
    pub(crate) fn resolve_sync(
        &self,
        id: ApprovalId,
        decision: ApprovalDecision,
        proof: PhysicalInputProof,
    ) -> Result<(), BrokerError> {
        let now = self.now();
        let mut st = self.lock();
        Self::expire(&mut st, now);
        let p = st
            .approvals
            .get_mut(&id)
            .ok_or(BrokerError::UnknownApproval(id))?;
        match p.status {
            PendStatus::Pending => {}
            PendStatus::Expired => return Err(BrokerError::ProofRejected("prośba wygasła".into())),
            _ => {
                return Err(BrokerError::ProofRejected(
                    "prośba już rozstrzygnięta".into(),
                ));
            }
        }
        Self::check_decision_shape(p, &decision)?;
        let holder = p.request.holder.clone();
        if let Err(reason) = Self::check_proof(p, &proof, now) {
            p.status = PendStatus::Denied;
            let payload = json!({ "approval": id, "result": "proof_rejected", "reason": reason });
            let _ = self.audit(EVENT_APPROVAL_DECIDED, Some(&holder), payload);
            return Err(BrokerError::ProofRejected(reason));
        }
        let payload = json!({ "approval": id, "decision": decision, "source": proof.source() });
        self.audit(EVENT_APPROVAL_DECIDED, Some(&holder), payload)?;
        self.apply_decision(&mut st, id, decision, now)
    }
}
