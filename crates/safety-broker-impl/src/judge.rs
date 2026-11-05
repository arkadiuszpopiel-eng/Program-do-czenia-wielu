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
                    model_location: req.facts.model_location,
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
        // Przegląd 5.11 (P1): warunki sprawdzane jeszcze raz przy werdykcie — w trakcie oceny właściciel mógł
        // obniżyć tryb albo wyłączyć sędziego, a bezpiecznik sesji mógł się otworzyć.
        let location = match &p.action {
            Some(PendingAction::Action(a)) => a.facts.model_location,
            _ => None,
        };
        let (rules, non_voice) = (p.request.rules.clone(), p.request.non_voice);
        let level =
            st.autonomy
                .effective_for(&holder.session, holder.agent.as_ref(), location, now);
        let still_judged = st.policy().judge.enabled
            && eligible(&rules, level, non_voice)
            && !st
                .judge_fuses
                .get(&holder.session)
                .is_some_and(|f| f.fuse.is_open());
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
        if self.record_fuse(&mut st, &holder, &verdict, resolution.task) {
            Self::to_owner_all(&mut st, &holder, id, FUSE_OPEN_NOTE);
        }
        match verdict {
            // Błąd mówi kierowcy (i dziennikowi werdyktów), że zgoda nie weszła w życie.
            JudgeVerdict::Allow { .. } if !still_judged => {
                Self::to_owner(&mut st, id, CHANGED_NOTE);
                Err(BrokerError::InvalidRequest(
                    "tryb albo ustawienia sędziego zmieniły się w trakcie oceny — decyduje właściciel"
                        .into(),
                ))
            }
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
            // Powód modelu (sterowalny treścią obcą) tylko w Audycie — na zaufanej karcie stały tekst (P4).
            JudgeVerdict::Escalate { .. } => {
                Self::to_owner(&mut st, id, ESCALATED_NOTE);
                Ok(())
            }
        }
    }

    /// Prośba `AwaitingJudge` (albo chwilowo `Pending`) → karta właściciela z dopiskiem.
    fn to_owner(st: &mut State, id: ApprovalId, note: &str) {
        if let Some(p) = st.approvals.get_mut(&id) {
            p.status = PendStatus::Pending;
            p.request.explanation.push(' ');
            p.request.explanation.push_str(note);
        }
    }

    /// Otwarty bezpiecznik: pozostałe prośby sesji czekające na sędziego idą do właściciela.
    fn to_owner_all(st: &mut State, holder: &Holder, except: ApprovalId, note: &str) {
        let ids: Vec<ApprovalId> = st
            .approvals
            .iter()
            .filter(|(id, p)| {
                **id != except
                    && p.request.holder.session == holder.session
                    && matches!(p.status, PendStatus::AwaitingJudge { .. })
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            Self::to_owner(st, id, note);
        }
    }

    /// Zapisuje werdykt w bezpieczniku; `true` — bezpiecznik właśnie się otworzył.
    fn record_fuse(
        &self,
        st: &mut State,
        holder: &Holder,
        verdict: &JudgeVerdict,
        task: Option<String>,
    ) -> bool {
        let f = fuse_for(st, holder);
        if task.is_some() && f.task != task {
            f.fuse.new_task();
            f.task = task;
        }
        if f.fuse.record(verdict) != FuseEvent::Tripped {
            return false;
        }
        let payload = json!({
            "open": true, "consecutive": f.fuse.consecutive(), "task_denials": f.fuse.task_denials(),
        });
        let _ = self.audit(EVENT_JUDGE_FUSE, Some(holder), payload);
        true
    }
}

const CHANGED_NOTE: &str =
    "Tryb pracy albo ustawienia sędziego zmieniły się w trakcie oceny — decydujesz Ty.";
const ESCALATED_NOTE: &str = "Sędzia zgód nie rozstrzygnął — decydujesz Ty (powód w dzienniku).";
const FUSE_OPEN_NOTE: &str = "Sędzia zgód odmówił kilka razy z rzędu — decydujesz Ty.";
