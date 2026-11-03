//! `Broker` i `KillSwitch` nad łączem IPC (rola `Core`): narzędzia agentek, Router zgód i UI
//! używają Brokera poza procesem tak samo jak silnika w procesie.
//!
//! Bezpieczny stan (fail-closed): gdy łącze jest zerwane albo Broker nie odpowiada, każda decyzja
//! kończy się `BrokerError::AuditUnavailable` (narzędzia: odmowa „Audyt niedostępny”), weryfikacja
//! tokenu — odrzuceniem, stan sesji — „skażona, z danymi prywatnymi”, poziom autonomii — ostatni
//! znany, najwyżej L1 (bez odczytu L0; [`crate::last_level`]).
//! Kill-switch zawsze zabija lokalne drzewa procesów narzędzi i wycisza audio, nawet bez Brokera.
//!
//! Stan zawężający, który Broker przyjął (skażenie sesji, obniżenia poziomu przez właściciela),
//! trafia do dziennika łącza ([`crate::replay::Journal`]) i jest odtwarzany w nowym procesie
//! Brokera po ponownym połączeniu (przegląd bezpieczeństwa #3, SR3-03).

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use core_bus_contract::{AgentId, Event, EventBus, Level, SessionId};
use platform_contract::{ProcessHandle, ProcessPort};
use risk_classifier_contract::{AutonomyLevel, CommandOrigin, KernelRule};
use safety_broker_contract::ipc::{Request, Response, UserChannel};
use safety_broker_contract::{
    ActionRequest, ApprovalId, ApprovalStatus, AttenuateRequest, AutonomyChangeRequest,
    AutonomyTarget, Broker, BrokerError, BrokerMetrics, CapToken, Capability, ChangeOrigin,
    ChildLockChange, ChildLockView, Decision, GrantInfo, Holder, KernelPolicy, PlanDecision,
    PlanRequest, SessionSecurity, TaintSource, TokenId, event_kind,
};
use serde_json::json;
use watchdog_contract::{
    EVENT_AUDIO_SILENCE, JobRecord, JobRegistry, JobTable, KillReason, KillReport, KillSwitch,
    ProcessRole,
};

use crate::answer::{answer, count, ok};
use crate::last_level::LastLevels;
use crate::link::BrokerLink;

/// Limit odpowiedzi Brokera na kill-switch (lokalne drzewa i audio są zatrzymane wcześniej).
pub const KILL_TIMEOUT: Duration = Duration::from_millis(500);

/// Stan sesji, gdy Broker jest niedostępny: najostrzejszy (taint + dane prywatne).
fn fail_closed_security() -> SessionSecurity {
    SessionSecurity {
        tainted: true,
        taint_sources: Vec::new(),
        private_data: true,
    }
}

/// Broker poza procesem.
#[derive(Debug, Clone)]
pub struct RemoteBroker {
    link: Arc<BrokerLink>,
    /// Karty podniesień poziomu czekające na decyzję (identyfikator → cel) — po zatwierdzeniu
    /// dziennik zapomina wcześniejsze obniżenie celu.
    raises: Arc<std::sync::Mutex<std::collections::BTreeMap<u64, AutonomyTarget>>>,
    /// Ostatnie poziomy z Sędziego — odpowiedź przy zerwanym łączu.
    levels: Arc<LastLevels>,
}

impl RemoteBroker {
    /// Broker nad łączem.
    pub fn new(link: Arc<BrokerLink>) -> Self {
        Self {
            link,
            raises: Arc::default(),
            levels: Arc::default(),
        }
    }
}

#[async_trait]
impl Broker for RemoteBroker {
    async fn decide(&self, action: ActionRequest) -> Result<Decision, BrokerError> {
        // Broker skaża sesję żądaniem z treści niezaufanej (jak `decide_sync`) — także po odmowie.
        let taints = action.origin == CommandOrigin::UntrustedContent
            || action.facts.untrusted_input_in_args;
        let session = action.holder.session.clone();
        let result = self.link.call_async(Request::Decide(action)).await;
        if taints && result.is_ok() {
            self.link.journal().taint(&session, &TaintSource::File);
        }
        answer(result, |r| match r {
            Response::Decision(d) => Some(d),
            _ => None,
        })
    }

    fn verify(
        &self,
        token: &CapToken,
        needed: &Capability,
        presenter: &Holder,
    ) -> Result<(), BrokerError> {
        let req = Request::Verify {
            token: token.clone(),
            needed: needed.clone(),
            presenter: presenter.clone(),
        };
        answer(self.link.call(req), ok)
    }

    async fn attenuate(
        &self,
        parent: &CapToken,
        presenter: &Holder,
        request: AttenuateRequest,
    ) -> Result<CapToken, BrokerError> {
        let req = Request::Attenuate {
            parent: parent.clone(),
            presenter: presenter.clone(),
            request,
        };
        answer(self.link.call_async(req).await, |r| match r {
            Response::Token(t) => Some(t),
            _ => None,
        })
    }

    async fn revoke(&self, id: TokenId) -> Result<usize, BrokerError> {
        answer(self.link.call_async(Request::Revoke { id }).await, count)
    }

    async fn revoke_holder(&self, holder: &Holder) -> Result<usize, BrokerError> {
        let req = Request::RevokeHolder {
            holder: holder.clone(),
        };
        answer(self.link.call_async(req).await, count)
    }

    async fn report_untrusted_input(
        &self,
        session: &SessionId,
        source: TaintSource,
    ) -> Result<(), BrokerError> {
        let req = Request::ReportUntrusted {
            session: session.clone(),
            source: source.clone(),
        };
        answer(self.link.call_async(req).await, ok)?;
        self.link.journal().taint(session, &source);
        Ok(())
    }

    fn session_security(&self, session: &SessionId) -> SessionSecurity {
        let req = Request::SessionSecurity {
            session: session.clone(),
        };
        match self.link.call(req) {
            Ok(Response::Security(s)) => {
                if s.tainted {
                    let journal = self.link.journal();
                    for t in &s.taint_sources {
                        journal.taint(session, t);
                    }
                    if s.taint_sources.is_empty() {
                        journal.taint(session, &TaintSource::File);
                    }
                }
                s
            }
            _ => fail_closed_security(),
        }
    }

    async fn submit_plan(&self, plan: PlanRequest) -> Result<PlanDecision, BrokerError> {
        answer(
            self.link.call_async(Request::SubmitPlan(plan)).await,
            |r| match r {
                Response::Plan(p) => Some(p),
                _ => None,
            },
        )
    }

    fn approval_status(
        &self,
        id: ApprovalId,
        requester: &Holder,
    ) -> Result<ApprovalStatus, BrokerError> {
        let req = Request::ApprovalStatus {
            id,
            requester: requester.clone(),
        };
        let status = answer(self.link.call(req), |r| match r {
            Response::Status(s) => Some(s),
            _ => None,
        })?;
        if !matches!(status, ApprovalStatus::Pending) {
            let mut raises = self.raises.lock().unwrap_or_else(|p| p.into_inner());
            let target = raises.remove(&id.0);
            if let (Some(target), ApprovalStatus::Approved { .. }) = (target, &status) {
                self.link.journal().raised(&target);
            }
        }
        Ok(status)
    }

    async fn request_autonomy_change(
        &self,
        request: AutonomyChangeRequest,
    ) -> Result<Option<ApprovalId>, BrokerError> {
        // Źródło zmiany ustala serwer z roli połączenia: jądro deklaruje tylko kanał właściciela.
        // Prośby agentki nie wolno przekazać jako zmiany właściciela (karta w Broker-UI
        // pokazywałaby fałszywe źródło) — wymaga procesu agentki z rolą `Agent` na potoku.
        let via = match request.origin {
            ChangeOrigin::UserInterface => UserChannel::UserInterface,
            ChangeOrigin::UserVoice => UserChannel::UserVoice,
            ChangeOrigin::Agent(_) => {
                return Err(BrokerError::KernelBlock(KernelRule::SelfEscalation));
            }
        };
        let (target, level, until_ms) = (request.target, request.level, request.until_ms);
        let req = Request::RequestAutonomy {
            target: target.clone(),
            level,
            until_ms,
            via,
        };
        let applied = answer(self.link.call_async(req).await, |r| match r {
            Response::Approval(id) => Some(id),
            _ => None,
        })?;
        // Zastosowane bez zgody = obniżenie właściciela (dziennik pamięta tylko poniżej L3).
        match applied {
            None => self.link.journal().lowered(&target, level, until_ms, via),
            Some(id) => {
                let mut raises = self.raises.lock().unwrap_or_else(|p| p.into_inner());
                // Karty żyją 10 min; niesprawdzane wpisy nie rosną bez końca.
                if raises.len() >= 256 {
                    raises.pop_first();
                }
                raises.insert(id.0, target);
            }
        }
        Ok(applied)
    }

    fn autonomy(&self, session: &SessionId, agent: Option<&AgentId>) -> AutonomyLevel {
        let req = Request::Autonomy {
            session: session.clone(),
            agent: agent.cloned(),
        };
        match self.link.call(req) {
            Ok(Response::Level(level)) => self.levels.remember(session, agent, level),
            _ => self.levels.offline(session, agent),
        }
    }

    async fn request_policy_change(
        &self,
        policy: KernelPolicy,
        origin: ChangeOrigin,
    ) -> Result<ApprovalId, BrokerError> {
        // Serwer przypisuje żądaniu jądra źródło „Ustawienia”; inne źródła nie mogą przez nie przejść.
        if origin != ChangeOrigin::UserInterface {
            return Err(BrokerError::KernelBlock(KernelRule::KernelPolicyChange));
        }
        let req = Request::RequestPolicy(Box::new(policy));
        answer(self.link.call_async(req).await, |r| match r {
            Response::Approval(Some(id)) => Some(id),
            _ => None,
        })
    }

    fn metrics(&self) -> BrokerMetrics {
        match self.link.call(Request::Metrics) {
            Ok(Response::Metrics(m)) => m,
            _ => BrokerMetrics::default(),
        }
    }

    fn grants(&self) -> Vec<GrantInfo> {
        match self.link.call(Request::Grants) {
            Ok(Response::Grants(g)) => g,
            _ => Vec::new(),
        }
    }

    async fn revoke_grant(&self, id: u64) -> Result<bool, BrokerError> {
        let req = Request::RevokeGrant { id };
        answer(self.link.call_async(req).await, |r| match r {
            Response::Revoked(done) => Some(done),
            _ => None,
        })
    }

    fn child_lock(&self) -> ChildLockView {
        match self.link.call(Request::ChildLock) {
            Ok(Response::ChildLock(v)) => v,
            // Sędzia nieosiągalny — stan nieznany; kłódkę i tak sprawdza Sędzia przy każdym działaniu.
            _ => ChildLockView {
                unknown: true,
                ..ChildLockView::default()
            },
        }
    }

    async fn change_child_lock(
        &self,
        change: ChildLockChange,
    ) -> Result<ChildLockView, BrokerError> {
        let req = Request::ChangeChildLock(change);
        answer(self.link.call_async(req).await, |r| match r {
            Response::ChildLock(v) => Some(v),
            _ => None,
        })
    }
}

/// Kill-switch po stronie aplikacji przy Brokerze poza procesem: drzewa procesów narzędzi
/// (uchwyty Job Objects należą do procesu aplikacji — zabija je ten sam port, który je
/// uruchomił), cisza audio na magistrali, potem `KillAll` w Brokerze (tokeny, prośby, plany).
pub struct RemoteKill {
    link: Arc<BrokerLink>,
    jobs: JobTable,
    processes: Arc<dyn ProcessPort>,
    bus: Option<Arc<dyn EventBus>>,
}

impl std::fmt::Debug for RemoteKill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteKill")
            .field("jobs", &self.jobs)
            .finish_non_exhaustive()
    }
}

impl RemoteKill {
    /// Kill-switch nad łączem, portem procesów narzędzi i magistralą (cisza audio).
    pub fn new(
        link: Arc<BrokerLink>,
        processes: Arc<dyn ProcessPort>,
        bus: Option<Arc<dyn EventBus>>,
    ) -> Self {
        Self {
            link,
            jobs: JobTable::default(),
            processes,
            bus,
        }
    }
}

#[async_trait]
impl KillSwitch for RemoteKill {
    async fn kill_all(&self, reason: KillReason) -> KillReport {
        let started = Instant::now();
        let (jobs_killed, jobs_failed) = self.jobs.kill_all(self.processes.as_ref());
        let audio_silenced = match &self.bus {
            Some(bus) => {
                let ev = Event::new(
                    event_kind(EVENT_AUDIO_SILENCE),
                    Level::Warn,
                    json!({ "reason": reason }),
                );
                bus.publish(ev).await.is_ok()
            }
            None => false,
        };
        let request = Request::KillAll {
            reason: reason.clone(),
        };
        let (tokens_revoked, audited) = match self
            .link
            .call_async_within(request, KILL_TIMEOUT)
            .await
        {
            Ok(Response::Killed(r)) => (r.tokens_revoked, r.audited),
            Ok(other) => {
                tracing::error!(odpowiedz = ?other, "kill-switch: nieoczekiwana odpowiedź Sędziego Cieni");
                (0, false)
            }
            Err(e) => {
                tracing::error!(error = %e, "kill-switch: Sędzia Cieni niedostępny (tokeny wygasną same)");
                (0, false)
            }
        };
        KillReport {
            reason,
            tokens_revoked,
            jobs_killed,
            jobs_failed,
            audio_silenced,
            audited,
            latency_us: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        }
    }
}

impl JobRegistry for RemoteKill {
    fn register_job(&self, job: ProcessHandle, owner: ProcessRole, label: &str) {
        self.jobs.register_job(job, owner, label);
    }

    fn unregister_job(&self, job: ProcessHandle) -> bool {
        self.jobs.unregister_job(job)
    }

    fn jobs(&self) -> Vec<JobRecord> {
        self.jobs.jobs()
    }
}
