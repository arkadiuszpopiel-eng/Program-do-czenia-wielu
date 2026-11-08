//! Atrapa Brokera (docs/modules/safety-broker/SPEC.md, sekcja „Fake”) do testów `agent-runtime`,
//! `tools-*`, UI.
//!
//! Pod spodem działa prawdziwy silnik `safety-broker-impl` z kluczami z jawnego ziarna (bez
//! sekretu — wyłącznie testy), Audytem w pamięci i portem procesów, który niczego nie zabija
//! naprawdę. Skrypt per narzędzie może wymusić `Allow`/`NeedsApproval`/`Deny`, ale **twarde
//! blokady Jądra działają zawsze**. [`FakeBroker::auto_approve`] udaje Broker-UI (syntetyczny
//! dowód z nonce wyzwania), jak `broker-ui-fake` ze SPEC.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod parts;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use compliance_contract::PathEnv;
use core_bus_contract::{AgentId, Event, SessionId};
use platform_contract::ProcessHandle;
use risk_classifier_contract::AutonomyLevel;
use safety_broker_contract::{
    ActionRequest, ApprovalChallenge, ApprovalChannel, ApprovalDecision, ApprovalId,
    ApprovalStatus, AttenuateRequest, AutonomyChangeRequest, Broker, BrokerError, BrokerMetrics,
    CapToken, Capability, ChangeOrigin, Decision, DenyReason, Holder, InputSource, KernelPolicy,
    PhysicalInputProof, PlanDecision, PlanRequest, SessionSecurity, TaintSource, TokenId,
    broker_ui_only,
};
use safety_broker_impl::audit::MemoryAudit;
use safety_broker_impl::{BrokerConfig, BrokerEngine, KeyMode};
use watchdog_contract::{
    Clock, JobRecord, JobRegistry, KillReason, KillReport, KillSwitch, ManualClock, ProcessRole,
};

pub use parts::{FakeProcesses, ScriptedClassifier, ScriptedDecision};

static NEXT_SEED: AtomicU64 = AtomicU64::new(1);

/// Atrapa Brokera.
pub struct FakeBroker {
    engine: BrokerEngine,
    audit: Arc<MemoryAudit>,
    processes: Arc<FakeProcesses>,
    script: Arc<ScriptedClassifier>,
    clock: Arc<ManualClock>,
    silences: Mutex<Vec<KillReason>>,
}

impl FakeBroker {
    /// Atrapa z polityką bazową dla profilu `C:\Users\user` i zegarem startującym od 1 000 000 ms.
    pub fn new() -> Result<Self, BrokerError> {
        let policy = KernelPolicy::baseline(r"C:\Users\user", r"C:\ProgramData\AlfaBroker")
            .map_err(|e| BrokerError::InvalidRequest(e.to_string()))?;
        Self::with(
            policy,
            PathEnv::windows_profile(r"C:\Users\user"),
            Arc::new(ManualClock::new(1_000_000)),
        )
    }

    /// Atrapa z własną polityką, środowiskiem i zegarem.
    pub fn with(
        policy: KernelPolicy,
        env: PathEnv,
        clock: Arc<ManualClock>,
    ) -> Result<Self, BrokerError> {
        let audit = Arc::new(MemoryAudit::default());
        let processes = Arc::new(FakeProcesses::default());
        let script = Arc::new(ScriptedClassifier::new(policy.risk));
        let seed = NEXT_SEED.fetch_add(1, Ordering::SeqCst);
        let config = BrokerConfig {
            policy,
            env,
            key_mode: KeyMode::Deterministic(seed),
        };
        let engine = BrokerEngine::new(config, clock.clone(), audit.clone(), processes.clone())?
            .with_classifier(script.clone());
        Ok(Self {
            engine,
            audit,
            processes,
            script,
            clock,
            silences: Mutex::new(Vec::new()),
        })
    }

    /// Skryptuje decyzję dla narzędzia (`facts.tool`); blokady Jądra mają pierwszeństwo.
    pub fn script(&self, tool: &str, decision: ScriptedDecision) {
        self.script.set(tool, decision);
    }

    /// Zegar atrapy.
    pub fn clock(&self) -> Arc<ManualClock> {
        self.clock.clone()
    }

    /// Zatwierdza wszystkie oczekujące prośby syntetycznym dowodem (rola Broker-UI w testach).
    pub async fn auto_approve(&self, decision: ApprovalDecision) -> usize {
        let mut n = 0;
        for ch in self.engine.pending() {
            let proof = broker_ui_only::physical_input_proof(
                ch.request.id,
                ch.nonce,
                InputSource::MouseClick,
                false,
                self.clock.now_ms(),
            );
            if self
                .engine
                .resolve(ch.request.id, decision.clone(), proof)
                .await
                .is_ok()
            {
                n += 1;
            }
        }
        n
    }

    /// Zdarzenia Audytu (w kolejności zapisu).
    pub fn audit_events(&self) -> Vec<Event> {
        self.audit.events()
    }

    /// Rodzaje zdarzeń Audytu.
    pub fn audit_names(&self) -> Vec<String> {
        self.audit.names()
    }

    /// Uchwyty „zabite” przez kill-switch.
    pub fn killed_jobs(&self) -> Vec<u32> {
        self.processes.killed()
    }

    /// Wysłane sygnały ciszy audio (powody kill-switcha).
    pub fn silences(&self) -> Vec<KillReason> {
        self.lock_silences().clone()
    }

    fn lock_silences(&self) -> MutexGuard<'_, Vec<KillReason>> {
        self.silences.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[async_trait]
impl Broker for FakeBroker {
    async fn decide(&self, action: ActionRequest) -> Result<Decision, BrokerError> {
        if let Some(ScriptedDecision::Deny(rule)) = self.script.get(&action.facts.tool) {
            return Ok(Decision::Deny(DenyReason::KernelBlock(rule)));
        }
        self.engine.decide(action).await
    }
    fn verify(&self, t: &CapToken, needed: &Capability, p: &Holder) -> Result<(), BrokerError> {
        self.engine.verify(t, needed, p)
    }
    async fn attenuate(
        &self,
        parent: &CapToken,
        presenter: &Holder,
        request: AttenuateRequest,
    ) -> Result<CapToken, BrokerError> {
        self.engine.attenuate(parent, presenter, request).await
    }
    async fn revoke(&self, id: TokenId) -> Result<usize, BrokerError> {
        self.engine.revoke(id).await
    }
    async fn revoke_holder(&self, holder: &Holder) -> Result<usize, BrokerError> {
        self.engine.revoke_holder(holder).await
    }
    async fn report_untrusted_input(
        &self,
        session: &SessionId,
        source: TaintSource,
    ) -> Result<(), BrokerError> {
        self.engine.report_untrusted_input(session, source).await
    }
    fn session_security(&self, session: &SessionId) -> SessionSecurity {
        self.engine.session_security(session)
    }
    async fn submit_plan(&self, plan: PlanRequest) -> Result<PlanDecision, BrokerError> {
        self.engine.submit_plan(plan).await
    }
    fn approval_status(&self, id: ApprovalId, r: &Holder) -> Result<ApprovalStatus, BrokerError> {
        self.engine.approval_status(id, r)
    }
    async fn request_autonomy_change(
        &self,
        request: AutonomyChangeRequest,
    ) -> Result<Option<ApprovalId>, BrokerError> {
        self.engine.request_autonomy_change(request).await
    }
    fn autonomy(&self, session: &SessionId, agent: Option<&AgentId>) -> AutonomyLevel {
        self.engine.autonomy(session, agent)
    }
    async fn request_policy_change(
        &self,
        policy: KernelPolicy,
        origin: ChangeOrigin,
    ) -> Result<ApprovalId, BrokerError> {
        self.engine.request_policy_change(policy, origin).await
    }
    fn metrics(&self) -> BrokerMetrics {
        self.engine.metrics()
    }
}

#[async_trait]
impl ApprovalChannel for FakeBroker {
    fn pending(&self) -> Vec<ApprovalChallenge> {
        self.engine.pending()
    }
    async fn resolve(
        &self,
        id: ApprovalId,
        decision: ApprovalDecision,
        proof: PhysicalInputProof,
    ) -> Result<(), BrokerError> {
        self.engine.resolve(id, decision, proof).await
    }
}

#[async_trait]
impl KillSwitch for FakeBroker {
    async fn kill_all(&self, reason: KillReason) -> KillReport {
        let mut report = self.engine.kill_all(reason.clone()).await;
        self.lock_silences().push(reason);
        report.audio_silenced = true;
        report
    }
}

impl JobRegistry for FakeBroker {
    fn register_job(&self, job: ProcessHandle, owner: ProcessRole, label: &str) {
        self.engine.register_job(job, owner, label);
    }
    fn unregister_job(&self, job: ProcessHandle) -> bool {
        self.engine.unregister_job(job)
    }
    fn jobs(&self) -> Vec<JobRecord> {
        self.engine.jobs()
    }
}
