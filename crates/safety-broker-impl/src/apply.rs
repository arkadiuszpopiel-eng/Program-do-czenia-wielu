//! Wykonanie zatwierdzonych próśb oraz żądania zmiany poziomu autonomii i polityk Jądra.

use risk_classifier_contract::{
    AutonomyLevel, CommandOrigin, KernelRule, Reversibility, RiskLevel,
};
use safety_broker_contract::{
    ApprovalDecision, ApprovalId, ApprovalSubject, AutonomyChangeRequest, AutonomyEntry,
    AutonomyTarget, BrokerError, ChangeOrigin, EVENT_AUTONOMY_CHANGED, EVENT_KERNEL_BLOCK,
    EVENT_POLICY_CHANGED, EVENT_TOKEN_ISSUED, HelloRequirement, Holder, KernelGuard, KernelPolicy,
    UNTIL_REVOKED,
};
use serde_json::json;

use crate::approvals::Meta;
use crate::engine::BrokerEngine;
use crate::grants::next_id;
use crate::state::{ApprovedPlan, Grant, PendStatus, PendingAction, State};

fn system_holder() -> Holder {
    Holder {
        session: core_bus_contract::SessionId::new("kernel"),
        agent: None,
        role: Some("owner".into()),
    }
}

/// Tryb po „Zezwól i przełącz na Automatycznie” (plan 50 §4.2).
const SWITCH_LEVEL: AutonomyLevel = AutonomyLevel::L3;

impl BrokerEngine {
    pub(crate) fn apply_decision(
        &self,
        st: &mut State,
        id: ApprovalId,
        decision: ApprovalDecision,
        now: u64,
    ) -> Result<(), BrokerError> {
        let Some(p) = st.approvals.get_mut(&id) else {
            return Err(BrokerError::UnknownApproval(id));
        };
        if decision == ApprovalDecision::Deny {
            p.status = PendStatus::Denied;
            return Ok(());
        }
        let action = p.action.take().ok_or(BrokerError::UnknownApproval(id))?;
        match self.apply_action(st, id, action, decision, now) {
            Ok(status) => {
                Self::set_status(st, id, status);
                Ok(())
            }
            Err(e) => {
                Self::set_status(st, id, PendStatus::Denied);
                Err(e)
            }
        }
    }

    fn apply_action(
        &self,
        st: &mut State,
        id: ApprovalId,
        action: PendingAction,
        decision: ApprovalDecision,
        now: u64,
    ) -> Result<PendStatus, BrokerError> {
        match action {
            PendingAction::Action(req) => {
                let session = st.session(&req.holder.session);
                if let Some(rule) = st.guard.derive_facts(&req, &session).kernel_rule {
                    return Err(BrokerError::KernelBlock(rule));
                }
                // Przed tokenem: błąd Audytu zmiany trybu = odmowa całości (fail-closed); sam tryb
                // zmienia się dopiero po zapisie tokenu (przegląd 3.11: odmowa nie zostawia L3).
                let switch = if decision == ApprovalDecision::AllowAndAutomatic {
                    self.audit_switch(st, &req.holder, id, now)?
                } else {
                    false
                };
                let ttl = Self::ttl(st, req.ttl_ms)?;
                let expires = now.saturating_add(ttl);
                let token = Self::mint(
                    st,
                    req.holder.clone(),
                    req.capability.clone(),
                    None,
                    expires,
                    now,
                );
                let payload = json!({ "token": token.id, "capability": token.cap, "via": "approval", "approval": id });
                self.audit(EVENT_TOKEN_ISSUED, Some(&req.holder), payload)?;
                Self::register(st, &token);
                if switch {
                    self.switch_to_automatic(st);
                }
                if let ApprovalDecision::AllowInScope { scope, until_ms } = decision {
                    // Termin przycinany do limitu polityki; „do odwołania” zostaje bez terminu
                    // (wybór właściciela w oknie — odwołanie w panelu uprawnień albo STOP).
                    let max = now.saturating_add(st.policy().grant_max_ms);
                    let until_ms = if until_ms == UNTIL_REVOKED {
                        UNTIL_REVOKED
                    } else {
                        until_ms.min(max)
                    };
                    let id = next_id(st, now);
                    st.grants.push(Grant {
                        id,
                        session: req.holder.session.clone(),
                        agent: req.holder.agent.clone(),
                        cap: scope,
                        granted_at_ms: now,
                        until_ms,
                    });
                    if until_ms == UNTIL_REVOKED {
                        self.persist_grants(st);
                    }
                }
                Ok(PendStatus::Approved(Some(Box::new(token))))
            }
            PendingAction::Plan {
                holder,
                origin,
                steps,
                ttl_ms,
            } => {
                let until_ms = now.saturating_add(ttl_ms.min(st.policy().plan_ttl_max_ms));
                st.plans.push(ApprovedPlan {
                    session: holder.session,
                    agent: holder.agent,
                    origin_kind: origin.kind(),
                    steps,
                    until_ms,
                });
                Ok(PendStatus::Approved(None))
            }
            PendingAction::Autonomy {
                target,
                level,
                until_ms,
            } => {
                let payload = json!({ "target": target, "level": level, "until_ms": until_ms, "approval": id });
                self.audit(EVENT_AUTONOMY_CHANGED, None, payload)?;
                // Podniesienie globalne bez terminu zostaje po restarcie (decyzja właściciela 27.10).
                if until_ms.is_none() {
                    self.remember(&target, level);
                }
                st.set_level(target, AutonomyEntry { level, until_ms });
                if level == AutonomyLevel::L4 {
                    Self::withdraw_full_auto(st, now);
                }
                Ok(PendStatus::Approved(None))
            }
            PendingAction::Policy(policy) => {
                policy.validate().map_err(BrokerError::InvalidRequest)?;
                let payload = json!({ "old": st.policy(), "new": policy, "approval": id });
                self.audit(EVENT_POLICY_CHANGED, None, payload)?;
                st.guard = KernelGuard::new(*policy, self.env.clone());
                Ok(PendStatus::Approved(None))
            }
        }
    }

    /// „Zezwól i przełącz na Automatycznie” (plan 50 §4.2): tryb globalny → L3, zapamiętany jak
    /// zmiana z menu. Gdy w międzyczasie podmiot dostał własny poziom albo tryb globalny jest
    /// już co najmniej L3 — sama zgoda, bez zmiany trybu.
    /// Czy wybór „Zezwól i przełącz na Automatycznie” zmienia tryb (podmiot nadal bez własnego
    /// poziomu, tryb globalny niższy niż L3) — wtedy zapis zmiany w Audycie.
    fn audit_switch(
        &self,
        st: &State,
        holder: &Holder,
        id: ApprovalId,
        now: u64,
    ) -> Result<bool, BrokerError> {
        let global = st
            .autonomy
            .get(&AutonomyTarget::Global, now)
            .unwrap_or_default();
        if global >= SWITCH_LEVEL
            || !st
                .autonomy
                .follows_global(&holder.session, holder.agent.as_ref(), now)
        {
            return Ok(false);
        }
        let target = AutonomyTarget::Global;
        let payload = json!({ "target": target, "level": SWITCH_LEVEL, "until_ms": null, "approval": id, "via": "approval_card" });
        self.audit(EVENT_AUTONOMY_CHANGED, None, payload)?;
        Ok(true)
    }

    /// Tryb globalny „Automatycznie” bez terminu, ze znacznikiem przełączenia z okna (rdzeń
    /// przyjmuje go jako zapamiętany).
    fn switch_to_automatic(&self, st: &mut State) {
        let target = AutonomyTarget::Global;
        self.remember(&target, SWITCH_LEVEL);
        st.set_level(
            target,
            AutonomyEntry {
                level: SWITCH_LEVEL,
                until_ms: None,
            },
        );
        st.switched_global = Some(SWITCH_LEVEL);
    }

    /// Po przejściu na „Pełną automatykę” okno zgód nie pokazuje się wcale (właściciel 31.10):
    /// czekające karty działań i planów podmiotów, które teraz mają L4, wygasają od razu —
    /// ponowione działanie przejdzie bez pytania (twarda blokada Jądra działa dalej).
    fn withdraw_full_auto(st: &mut State, now: u64) {
        let full = |st: &State, h: &Holder, location| {
            st.autonomy
                .effective_for(&h.session, h.agent.as_ref(), location, now)
                == AutonomyLevel::L4
        };
        let ids: Vec<ApprovalId> = st
            .approvals
            .iter()
            .filter(|(_, p)| matches!(p.status, PendStatus::Pending))
            .filter(|(_, p)| {
                matches!(
                    p.action,
                    Some(PendingAction::Action(_) | PendingAction::Plan { .. })
                )
            })
            .filter(|(_, p)| {
                let location = match &p.action {
                    Some(PendingAction::Action(a)) => a.facts.model_location,
                    _ => None,
                };
                full(st, &p.request.holder, location)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            Self::set_status(st, id, PendStatus::Expired);
        }
    }

    fn set_status(st: &mut State, id: ApprovalId, status: PendStatus) {
        if let Some(p) = st.approvals.get_mut(&id) {
            p.status = status;
        }
    }

    fn origin_is_agent(origin: &ChangeOrigin) -> bool {
        matches!(origin, ChangeOrigin::Agent(_))
    }

    /// `Broker::request_autonomy_change`.
    pub(crate) fn autonomy_sync(
        &self,
        req: AutonomyChangeRequest,
    ) -> Result<Option<ApprovalId>, BrokerError> {
        let now = self.now();
        let mut st = self.lock();
        let current = st.current_level(&req.target, now);
        if req.until_ms.is_some_and(|u| u <= now) {
            return Err(BrokerError::InvalidRequest(
                "termin zmiany poziomu w przeszłości".into(),
            ));
        }
        // Wpis celu już jest dokładnie taki (poziom i termin) — nic się nie zmienia.
        let unchanged = st.autonomy.entries().iter().any(|(t, e)| {
            *t == req.target
                && e.level == req.level
                && e.until_ms == req.until_ms
                && e.until_ms.is_none_or(|u| now < u)
        });
        if unchanged {
            return Ok(None);
        }
        // Tylko poziom ściśle niższy jest obniżeniem. Równy poziom bez terminu (albo z dłuższym)
        // utrwala czasowe L4 właściciela — to podniesienie w czasie (regresja SR-02).
        if req.level < current {
            // Obniżenie „na czas” nie może po wygaśnięciu skończyć się poziomem wyższym niż
            // przed żądaniem (np. nadpisanie jawnego L2 właściciela krótkim L1 → powrót do L4).
            let fallback = st.fallback_level(&req.target, now);
            let until_ms = req.until_ms.filter(|_| fallback <= current);
            let entry = AutonomyEntry {
                level: req.level,
                until_ms,
            };
            if until_ms.is_none() {
                self.remember(&req.target, req.level);
            }
            st.set_level(req.target.clone(), entry);
            let payload = json!({ "target": req.target, "from": current, "level": req.level, "origin": req.origin });
            self.audit(EVENT_AUTONOMY_CHANGED, None, payload)?;
            return Ok(None);
        }
        if Self::origin_is_agent(&req.origin) {
            st.kernel_blocks += 1;
            let payload = json!({ "rule": KernelRule::SelfEscalation, "target": req.target, "level": req.level, "origin": req.origin });
            let _ = self.audit(EVENT_KERNEL_BLOCK, None, payload);
            return Err(BrokerError::KernelBlock(KernelRule::SelfEscalation));
        }
        let hello = req.level == AutonomyLevel::L4
            && st
                .policy()
                .hello_required_for
                .contains(&HelloRequirement::L4);
        let subject = ApprovalSubject::Autonomy {
            target: req.target.clone(),
            from: current,
            to: req.level,
            until_ms: req.until_ms,
        };
        let origin = if req.origin == ChangeOrigin::UserVoice {
            CommandOrigin::UserVoice {
                confidence: risk_classifier_contract::SttConfidence::from_permille(0),
                speaker_verified: false,
            }
        } else {
            CommandOrigin::UserText
        };
        let meta = Meta {
            holder: system_holder(),
            risk: if req.level == AutonomyLevel::L4 {
                RiskLevel::High
            } else {
                RiskLevel::Medium
            },
            reversible: Reversibility::Yes,
            origin,
            tainted: false,
            non_voice: true,
            grantable: false,
            hello_required: hello,
            rules: Vec::new(),
            explanation: format!("Zmiana poziomu autonomii: {current} → {}.", req.level),
            autonomy: None,
        };
        let action = PendingAction::Autonomy {
            target: req.target,
            level: req.level,
            until_ms: req.until_ms,
        };
        self.open_approval(&mut st, subject, meta, action, now)
            .map(|t| Some(t.id))
    }

    /// `Broker::request_policy_change`.
    pub(crate) fn policy_sync(
        &self,
        policy: KernelPolicy,
        origin: ChangeOrigin,
    ) -> Result<ApprovalId, BrokerError> {
        let now = self.now();
        let mut st = self.lock();
        if Self::origin_is_agent(&origin) {
            st.kernel_blocks += 1;
            let payload = json!({ "rule": KernelRule::KernelPolicyChange, "origin": origin });
            let _ = self.audit(EVENT_KERNEL_BLOCK, None, payload);
            return Err(BrokerError::KernelBlock(KernelRule::KernelPolicyChange));
        }
        policy.validate().map_err(BrokerError::InvalidRequest)?;
        let hello = st
            .policy()
            .hello_required_for
            .contains(&HelloRequirement::Policy);
        let meta = Meta {
            holder: system_holder(),
            risk: RiskLevel::High,
            reversible: Reversibility::Yes,
            origin: CommandOrigin::UserText,
            tainted: false,
            non_voice: true,
            grantable: false,
            hello_required: hello,
            rules: Vec::new(),
            explanation: "Zmiana polityk Jądra — sprawdź szczegóły przed zatwierdzeniem.".into(),
            autonomy: None,
        };
        let subject = ApprovalSubject::Policy {
            policy: Box::new(policy.clone()),
        };
        let action = PendingAction::Policy(Box::new(policy));
        self.open_approval(&mut st, subject, meta, action, now)
            .map(|t| t.id)
    }
}
