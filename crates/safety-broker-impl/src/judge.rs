//! Model-sędzia trybu „Automatycznie” (plan 50 §4.3, Z.4): prośba L3 czeka na werdykt kierowcy w jądrze,
//! zamiast od razu trafić na kartę właściciela. Sędzia daje co najwyżej zgodę jednorazową; termin, brak
//! kierowcy, otwarty bezpiecznik i każdy błąd kończą się kartą właściciela.

use approval_judge_contract::{FuseEvent, JudgeFuse, JudgeVerdict, eligible};
use risk_classifier_contract::{AutonomyLevel, RuleId};
use safety_broker_contract::{
    ApprovalDecision, ApprovalId, BrokerError, EVENT_JUDGE_DECIDED, EVENT_JUDGE_FUSE,
    EVENT_JUDGE_REQUESTED, EVENT_JUDGE_TIMEOUT, Holder, JUDGE_DRIVER_STALE_MS, JudgeRequest,
    JudgeResolution,
};
use serde_json::json;

use crate::engine::BrokerEngine;
use crate::state::{PendStatus, PendingAction, SessionFuse, State};

fn fuse_for<'a>(st: &'a mut State, holder: &Holder) -> &'a mut SessionFuse {
    let limits = st.policy().judge.fuse_limits();
    st.judge_fuses
        .entry(holder.session.clone())
        .or_insert_with(|| SessionFuse {
            fuse: JudgeFuse::new(limits),
            task: None,
        })
}

impl BrokerEngine {
    /// Czy prośbę o akcję oddać sędziemu: sędzia włączony, kierowca żywy, poziom L3, reguły z listy,
    /// bezpiecznik sesji zamknięty.
    pub(crate) fn routes_to_judge(
        st: &State,
        holder: &Holder,
        level: AutonomyLevel,
        rules: &[RuleId],
        non_voice: bool,
        now: u64,
    ) -> bool {
        st.policy().judge.enabled
            && st
                .judge_seen_ms
                .is_some_and(|t| now.saturating_sub(t) <= JUDGE_DRIVER_STALE_MS)
            && eligible(rules, level, non_voice)
            && !st
                .judge_fuses
                .get(&holder.session)
                .is_some_and(|f| f.fuse.is_open())
    }

    /// Przestawia świeżo otwartą prośbę na „czeka na sędziego”. Bez zapisu w Audycie zostaje u właściciela.
    pub(crate) fn hand_to_judge(&self, st: &mut State, id: ApprovalId, now: u64) {
        let deadline_ms = now.saturating_add(st.policy().judge.timeout_ms);
        let Some(holder) = st.approvals.get(&id).map(|p| p.request.holder.clone()) else {
            return;
        };
        let payload = json!({ "approval": id, "deadline_ms": deadline_ms });
        if self
            .audit(EVENT_JUDGE_REQUESTED, Some(&holder), payload)
            .is_ok()
            && let Some(p) = st.approvals.get_mut(&id)
            && matches!(p.status, PendStatus::Pending)
        {
            p.status = PendStatus::AwaitingJudge { deadline_ms };
        }
    }

    /// Prośby, na które sędzia nie zdążył, przechodzą na kartę właściciela (bezpieczny kierunek — także
    /// gdy zapis w Audycie się nie uda).
    pub(crate) fn promote_judge_timeouts(&self, st: &mut State, now: u64) {
        let late: Vec<(ApprovalId, Holder)> = st
            .approvals
            .iter()
            .filter(|(_, p)| {
                matches!(p.status, PendStatus::AwaitingJudge { deadline_ms } if now >= deadline_ms)
            })
            .map(|(id, p)| (*id, p.request.holder.clone()))
            .collect();
        for (id, holder) in late {
            if let Some(p) = st.approvals.get_mut(&id) {
                p.status = PendStatus::Pending;
                p.request
                    .explanation
                    .push_str(" Sędzia nie zdążył z oceną — decydujesz Ty.");
            }
            let _ = self.audit(
                EVENT_JUDGE_TIMEOUT,
                Some(&holder),
                json!({ "approval": id }),
            );
        }
    }

    /// Decyzja właściciela na karcie: zgoda zamyka otwarty bezpiecznik sesji.
    pub(crate) fn owner_decided_fuse(&self, st: &mut State, holder: &Holder, approved: bool) {
        let Some(f) = st.judge_fuses.get_mut(&holder.session) else {
            return;
        };
        if f.fuse.owner_decided(approved) == FuseEvent::Reset {
            let _ = self.audit(EVENT_JUDGE_FUSE, Some(holder), json!({ "open": false }));
        }
    }

    /// `Broker::judge_queue` — także znak życia kierowcy.
    pub(crate) fn judge_queue_sync(&self) -> Vec<JudgeRequest> {
        let now = self.now();
        let mut st = self.lock();
        st.judge_seen_ms = Some(now);
        Self::expire(&mut st, now);
        self.promote_judge_timeouts(&mut st, now);
        let st = &*st;
        st.approvals
            .values()
            .filter_map(|p| {
                let PendStatus::AwaitingJudge { deadline_ms } = p.status else {
                    return None;
                };
                let Some(PendingAction::Action(req)) = &p.action else {
                    return None;
                };
                let facts = st.guard.derive_facts(req, &st.session(&req.holder.session));
                Some(JudgeRequest {
                    approval: p.request.id,
                    holder: req.holder.clone(),
                    tool: req.facts.tool.clone(),
                    capability: req.capability.clone(),
                    summary: req.facts.summary.clone(),
                    command: req.facts.command.clone(),
                    workdir: req.facts.workdir.clone(),
                    reversible: req.facts.reversible,
                    destructive: req.facts.destructive,
                    rules: p.request.rules.clone(),
                    tainted: facts.tainted,
                    outside_workdir: facts.outside_workdir,
                    host: facts.egress.clone(),
                    deadline_ms,
                })
            })
            .collect()
    }

    /// `Broker::judge_resolve`.
    pub(crate) fn judge_resolve_sync(
        &self,
        id: ApprovalId,
        resolution: JudgeResolution,
    ) -> Result<(), BrokerError> {
        let now = self.now();
        let mut st = self.lock();
        Self::expire(&mut st, now);
        self.promote_judge_timeouts(&mut st, now);
        let p = st
            .approvals
            .get(&id)
            .ok_or(BrokerError::UnknownApproval(id))?;
        if !matches!(p.status, PendStatus::AwaitingJudge { .. }) {
            return Err(BrokerError::InvalidRequest(
                "prośba nie czeka na sędziego (termin minął albo rozstrzygnięta)".into(),
            ));
        }
        let holder = p.request.holder.clone();
        let verdict = resolution.verdict;
        let payload = json!({
            "approval": id, "verdict": verdict, "model": resolution.model,
            "took_ms": resolution.took_ms, "task": resolution.task,
        });
        if let Err(e) = self.audit(EVENT_JUDGE_DECIDED, Some(&holder), payload) {
            // Bez zapisu werdyktu: odmowa obowiązuje (jak u właściciela), reszta idzie do właściciela.
            let status = match &verdict {
                JudgeVerdict::Deny { category, reason } => {
                    PendStatus::DeniedByJudge(format!("{}: {reason}", category.name_pl()))
                }
                _ => PendStatus::Pending,
            };
            if let Some(p) = st.approvals.get_mut(&id) {
                p.status = status;
            }
            return Err(e);
        }
        self.record_fuse(&mut st, &holder, &verdict, resolution.task);
        match verdict {
            JudgeVerdict::Allow { .. } => {
                if let Some(p) = st.approvals.get_mut(&id) {
                    p.status = PendStatus::Pending;
                }
                self.apply_decision(&mut st, id, ApprovalDecision::Allow, now)
            }
            JudgeVerdict::Deny { category, reason } => {
                if let Some(p) = st.approvals.get_mut(&id) {
                    p.status =
                        PendStatus::DeniedByJudge(format!("{}: {reason}", category.name_pl()));
                }
                Ok(())
            }
            JudgeVerdict::Escalate { reason } => {
                if let Some(p) = st.approvals.get_mut(&id) {
                    p.status = PendStatus::Pending;
                    p.request
                        .explanation
                        .push_str(&format!(" Sędzia przekazał decyzję Tobie: {reason}"));
                }
                Ok(())
            }
        }
    }

    fn record_fuse(
        &self,
        st: &mut State,
        holder: &Holder,
        verdict: &JudgeVerdict,
        task: Option<String>,
    ) {
        let f = fuse_for(st, holder);
        if task.is_some() && f.task != task {
            f.fuse.new_task();
            f.task = task;
        }
        if f.fuse.record(verdict) == FuseEvent::Tripped {
            let payload = json!({
                "open": true, "consecutive": f.fuse.consecutive(), "task_denials": f.fuse.task_denials(),
            });
            let _ = self.audit(EVENT_JUDGE_FUSE, Some(holder), payload);
        }
    }
}
