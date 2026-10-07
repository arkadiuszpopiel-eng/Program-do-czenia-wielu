//! Decyzja o akcji i plan do zatwierdzenia.

use risk_classifier_contract::{CommandOrigin, RiskLevel, RiskVerdict, Verdict};
use safety_broker_contract::{
    ActionRequest, ApprovalSubject, BrokerError, Decision, DenyReason, EVENT_KERNEL_BLOCK,
    EVENT_SESSION_TAINTED, EVENT_TOKEN_ISSUED, PlanDecision, PlanRequest, PlanStepSummary,
    TaintSource,
};
use serde_json::json;

use crate::engine::BrokerEngine;
use crate::state::{ApprovedStep, PendingAction, State};

/// Maksymalna liczba kroków planu.
const MAX_PLAN_STEPS: usize = 64;

impl BrokerEngine {
    /// Oznacza sesję jako `tainted` (monotonicznie); zapis do Audytu przy pierwszym oznaczeniu
    /// danym źródłem.
    pub(crate) fn taint(
        &self,
        st: &mut State,
        req_holder: &safety_broker_contract::Holder,
        source: TaintSource,
    ) {
        let entry = st.sessions.entry(req_holder.session.clone()).or_default();
        let new = !entry.taint_sources.contains(&source);
        entry.tainted = true;
        if new {
            entry.taint_sources.push(source.clone());
            let payload = json!({ "source": source });
            // Taint działa także bez zapisu (bezpieczniej); błąd Audytu nie cofa oznaczenia.
            let _ = self.audit(EVENT_SESSION_TAINTED, Some(req_holder), payload);
        }
    }

    fn kernel_block(
        &self,
        st: &mut State,
        req: &ActionRequest,
        rule: risk_classifier_contract::KernelRule,
    ) -> Decision {
        st.kernel_blocks += 1;
        let payload = json!({ "rule": rule, "capability": req.capability, "tool": req.facts.tool });
        // Odmowa jest bezpieczna także wtedy, gdy Audyt zawiódł.
        let _ = self.audit(EVENT_KERNEL_BLOCK, Some(&req.holder), payload);
        Decision::Deny(DenyReason::KernelBlock(rule))
    }

    fn issue_for(
        &self,
        st: &mut State,
        req: &ActionRequest,
        verdict: &RiskVerdict,
        via: &str,
        now: u64,
    ) -> Result<Decision, BrokerError> {
        let ttl = Self::ttl(st, req.ttl_ms)?;
        let token = Self::mint(
            st,
            req.holder.clone(),
            req.capability.clone(),
            None,
            now.saturating_add(ttl),
            now,
        );
        let payload = json!({
            "token": token.id, "capability": token.cap, "via": via, "ttl_ms": ttl,
            "risk": verdict.level, "rules": verdict.rules, "tool": req.facts.tool,
        });
        if self
            .audit(EVENT_TOKEN_ISSUED, Some(&req.holder), payload)
            .is_err()
        {
            return Ok(Decision::Deny(DenyReason::AuditUnavailable));
        }
        Self::register(st, &token);
        Ok(Decision::Allow(token))
    }

    /// `Broker::decide` (synchronicznie, pod zamkiem stanu).
    pub(crate) fn decide_sync(&self, req: ActionRequest) -> Result<Decision, BrokerError> {
        Self::check_holder(&req.holder)?;
        Self::ttl(&self.lock(), req.ttl_ms)?;
        let now = self.now();
        let mut st = self.lock();
        if req.origin == CommandOrigin::UntrustedContent || req.facts.untrusted_input_in_args {
            self.taint(&mut st, &req.holder, TaintSource::File);
        }
        let session = st.session(&req.holder.session);
        let facts = st.guard.derive_facts(&req, &session);
        let level = st
            .autonomy
            .effective(&req.holder.session, req.holder.agent.as_ref(), now);
        let verdict = self.classify(&st, &facts, level);
        match verdict.verdict {
            Verdict::HardBlock { rule } => Ok(self.kernel_block(&mut st, &req, rule)),
            Verdict::Proceed => self.issue_for(&mut st, &req, &verdict, "autonomy", now),
            Verdict::Ask {
                non_voice,
                grantable,
            } => {
                let by_plan = st.plans.iter().any(|p| {
                    now < p.until_ms
                        && p.session == req.holder.session
                        && p.agent == req.holder.agent
                        && p.origin_kind == req.origin.kind()
                        && p.steps
                            .iter()
                            .any(|s| s.covers(&req.capability, &req.facts, &verdict.rules))
                });
                if by_plan {
                    return self.issue_for(&mut st, &req, &verdict, "plan", now);
                }
                let by_grant = grantable
                    && st.grants.iter().any(|g| {
                        now < g.until_ms
                            && g.session == req.holder.session
                            && g.agent == req.holder.agent
                            && req.capability.is_subset_of(&g.cap)
                    });
                if by_grant {
                    return self.issue_for(&mut st, &req, &verdict, "grant", now);
                }
                let subject = ApprovalSubject::Action {
                    capability: req.capability.clone(),
                    tool: req.facts.tool.clone(),
                };
                let meta = crate::approvals::Meta {
                    holder: req.holder.clone(),
                    risk: verdict.level,
                    reversible: req.facts.reversible,
                    origin: req.origin,
                    tainted: facts.tainted,
                    non_voice,
                    grantable,
                    hello_required: false,
                    rules: verdict.rules.clone(),
                    explanation: verdict.explanation.clone(),
                };
                let action = PendingAction::Action(Box::new(req));
                match self.open_approval(&mut st, subject, meta, action, now) {
                    Ok(ticket) => Ok(Decision::NeedsApproval(ticket)),
                    Err(BrokerError::AuditUnavailable(_)) => {
                        Ok(Decision::Deny(DenyReason::AuditUnavailable))
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    /// `Broker::submit_plan`.
    pub(crate) fn submit_plan_sync(&self, plan: PlanRequest) -> Result<PlanDecision, BrokerError> {
        Self::check_holder(&plan.holder)?;
        if plan.steps.is_empty() || plan.steps.len() > MAX_PLAN_STEPS || plan.ttl_ms == 0 {
            return Err(BrokerError::InvalidRequest(format!(
                "plan musi mieć 1–{MAX_PLAN_STEPS} kroków i dodatni czas ważności"
            )));
        }
        let now = self.now();
        let mut st = self.lock();
        let session = st.session(&plan.holder.session);
        let level = st
            .autonomy
            .effective(&plan.holder.session, plan.holder.agent.as_ref(), now);
        let mut summaries = Vec::new();
        let mut shown = Vec::new();
        let mut meta_rules = Vec::new();
        let (mut risk, mut non_voice, mut tainted) = (RiskLevel::Low, false, session.tainted);
        for (i, step) in plan.steps.iter().enumerate() {
            let req = ActionRequest {
                holder: plan.holder.clone(),
                capability: step.capability.clone(),
                facts: step.facts.clone(),
                origin: plan.origin,
                ttl_ms: None,
            };
            let facts = st.guard.derive_facts(&req, &session);
            let v = self.classify(&st, &facts, level);
            match v.verdict {
                Verdict::HardBlock { rule } => {
                    self.kernel_block(&mut st, &req, rule);
                    return Ok(PlanDecision::Rejected { step: i, rule });
                }
                Verdict::Ask { non_voice: nv, .. } => {
                    risk = risk.max(v.level);
                    non_voice |= nv;
                    tainted |= facts.tainted;
                    meta_rules.extend(v.rules.iter().copied());
                    summaries.push(PlanStepSummary {
                        capability: step.capability.clone(),
                        description: step.description.clone(),
                        risk: v.level,
                    });
                    shown.push(ApprovedStep {
                        step: step.clone(),
                        rules: v.rules.clone(),
                    });
                }
                Verdict::Proceed => {}
            }
        }
        if summaries.is_empty() {
            // Nic nie wymaga zgody: plan niczego nie zapisuje — kroki przejdą same, dopóki nie
            // zmienią się warunki (taint, poziom). Zapisany plan „wyprałby” późniejsze reguły
            // każdego poziomu bez wiedzy właściciela (regresja SR-01 w `tests/review.rs`).
            return Ok(PlanDecision::Approved);
        }
        meta_rules.dedup();
        let asked = summaries.len();
        let subject = ApprovalSubject::Plan {
            title: plan.title.clone(),
            steps: summaries,
        };
        let meta = crate::approvals::Meta {
            holder: plan.holder.clone(),
            risk,
            reversible: plan
                .steps
                .iter()
                .map(|s| s.facts.reversible)
                .max_by_key(|r| *r as u8)
                .unwrap_or(risk_classifier_contract::Reversibility::Yes),
            origin: plan.origin,
            tainted,
            non_voice,
            grantable: false,
            hello_required: false,
            explanation: format!("Plan „{}”: kroków wymagających zgody: {asked}.", plan.title),
            rules: meta_rules,
        };
        let action = PendingAction::Plan {
            holder: plan.holder,
            origin: plan.origin,
            steps: shown,
            ttl_ms: plan.ttl_ms,
        };
        self.open_approval(&mut st, subject, meta, action, now)
            .map(PlanDecision::NeedsApproval)
    }
}
