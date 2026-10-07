//! Testy kontraktowe Brokera (`safety-broker-contract`, feature `contract-tests`) na Brokerze
//! poza procesem: strona agentek przez `RemoteBroker` (rola `Core` po tożsamości obrazu),
//! kanał zatwierdzeń przez rolę `BrokerUi` (poświadczenie z MAC), kill-switch przez `RemoteKill`
//! (lokalne drzewa narzędzi + cisza audio + `KillAll` w Brokerze) — wszystko przez prawdziwą
//! usługę (`safety-broker-impl::service`) na potokach z ACL (`platform-fake`).
//!
//! Świadome odstępstwo (opisane w SPEC): prośbę agentki o zmianę poziomu jądro przekazuje jako
//! `KernelBlock(SelfEscalation)` także przy obniżeniu — rola `Core` nie umie wyrazić źródła
//! „agentka” i nie wolno jej podać jako zmiany właściciela.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};

use app_broker::{RemoteBroker, RemoteKill};
use async_trait::async_trait;
use core_bus_contract::{AgentId, SessionId};
use platform_contract::{PipeConnection, ProcessHandle, SecurePipePort};
use platform_fake::FakePipes;
use safety_broker_contract::contract_tests::{self as ct, FullBroker};
use safety_broker_contract::ipc::{
    ClientRole, Hello, PROTOCOL_VERSION, ProofWire, Request, Response,
};
use safety_broker_contract::ipc_blocking::BlockingClient;
use safety_broker_contract::{
    ActionRequest, ApprovalChallenge, ApprovalChannel, ApprovalDecision, ApprovalId,
    ApprovalStatus, AttenuateRequest, AutonomyChangeRequest, AutonomyLevel, AutonomyTarget, Broker,
    BrokerError, BrokerMetrics, CapToken, Capability, ChangeOrigin, Decision, Holder, KernelPolicy,
    KernelRule, PhysicalInputProof, PlanDecision, PlanRequest, SessionSecurity, TaintSource,
    TokenId,
};
use watchdog_contract::{JobRecord, JobRegistry, KillReason, KillReport, KillSwitch, ManualClock};

/// Broker poza procesem złożony z ról IPC.
struct Remote {
    broker: RemoteBroker,
    kill: RemoteKill,
    ui: Mutex<BlockingClient<Box<dyn PipeConnection>>>,
    _service: common::Service,
}

fn remote(policy: KernelPolicy, clock: Arc<ManualClock>) -> Remote {
    let sys = FakePipes::default();
    for id in common::identities() {
        sys.register(id);
    }
    let server = Arc::new(sys.process(common::SERVER_PID));
    let service = common::service(server.clone(), server, policy, clock, None);
    let app = Arc::new(sys.process(common::CORE_PID));
    let (_kernel, link) = common::app_kernel(app.clone(), app, false);
    let processes = Arc::new(safety_broker_fake::FakeProcesses::default());
    let bus: Arc<dyn core_bus_contract::EventBus> = Arc::new(core_bus_fake::FakeBus::default());
    let credential =
        service
            .engine
            .issue_client_credential("broker-ui-test", ClientRole::BrokerUi, 3_600_000);
    let conn = sys
        .process(common::UI_PID)
        .connect(common::PIPE, 100)
        .unwrap();
    let hello = Hello {
        protocol: PROTOCOL_VERSION,
        credential,
        pid: common::UI_PID,
        sid: None,
        image: None,
    };
    Remote {
        broker: RemoteBroker::new(link.clone()),
        kill: RemoteKill::new(link, processes, Some(bus)),
        ui: Mutex::new(BlockingClient::connect(conn, &hello).unwrap()),
        _service: service,
    }
}

#[async_trait]
impl Broker for Remote {
    async fn decide(&self, a: ActionRequest) -> Result<Decision, BrokerError> {
        self.broker.decide(a).await
    }
    fn verify(&self, t: &CapToken, n: &Capability, p: &Holder) -> Result<(), BrokerError> {
        self.broker.verify(t, n, p)
    }
    async fn attenuate(
        &self,
        t: &CapToken,
        p: &Holder,
        r: AttenuateRequest,
    ) -> Result<CapToken, BrokerError> {
        self.broker.attenuate(t, p, r).await
    }
    async fn revoke(&self, id: TokenId) -> Result<usize, BrokerError> {
        self.broker.revoke(id).await
    }
    async fn revoke_holder(&self, h: &Holder) -> Result<usize, BrokerError> {
        self.broker.revoke_holder(h).await
    }
    async fn report_untrusted_input(
        &self,
        s: &SessionId,
        src: TaintSource,
    ) -> Result<(), BrokerError> {
        self.broker.report_untrusted_input(s, src).await
    }
    fn session_security(&self, s: &SessionId) -> SessionSecurity {
        self.broker.session_security(s)
    }
    async fn submit_plan(&self, p: PlanRequest) -> Result<PlanDecision, BrokerError> {
        self.broker.submit_plan(p).await
    }
    fn approval_status(&self, id: ApprovalId, h: &Holder) -> Result<ApprovalStatus, BrokerError> {
        self.broker.approval_status(id, h)
    }
    async fn request_autonomy_change(
        &self,
        r: AutonomyChangeRequest,
    ) -> Result<Option<ApprovalId>, BrokerError> {
        self.broker.request_autonomy_change(r).await
    }
    fn autonomy(&self, s: &SessionId, a: Option<&AgentId>) -> AutonomyLevel {
        self.broker.autonomy(s, a)
    }
    async fn request_policy_change(
        &self,
        p: KernelPolicy,
        o: ChangeOrigin,
    ) -> Result<ApprovalId, BrokerError> {
        self.broker.request_policy_change(p, o).await
    }
    fn metrics(&self) -> BrokerMetrics {
        self.broker.metrics()
    }
}

#[async_trait]
impl ApprovalChannel for Remote {
    fn pending(&self) -> Vec<ApprovalChallenge> {
        match self.ui.lock().unwrap().call(Request::PendingApprovals) {
            Ok(Response::Pending(list)) => list,
            other => panic!("{other:?}"),
        }
    }

    async fn resolve(
        &self,
        id: ApprovalId,
        decision: ApprovalDecision,
        proof: PhysicalInputProof,
    ) -> Result<(), BrokerError> {
        let proof = ProofWire {
            approval: proof.approval(),
            nonce: proof.nonce(),
            source: proof.source(),
            injected: proof.injected(),
            at_ms: proof.at_ms(),
        };
        let req = Request::Resolve {
            id,
            decision,
            proof,
        };
        match self.ui.lock().unwrap().call(req) {
            Ok(Response::Ok) => Ok(()),
            Ok(Response::Error(e)) => Err(e),
            other => panic!("{other:?}"),
        }
    }
}

#[async_trait]
impl KillSwitch for Remote {
    async fn kill_all(&self, reason: KillReason) -> KillReport {
        self.kill.kill_all(reason).await
    }
}

impl JobRegistry for Remote {
    fn register_job(&self, job: ProcessHandle, owner: watchdog_contract::ProcessRole, l: &str) {
        self.kill.register_job(job, owner, l);
    }
    fn unregister_job(&self, job: ProcessHandle) -> bool {
        self.kill.unregister_job(job)
    }
    fn jobs(&self) -> Vec<JobRecord> {
        self.kill.jobs()
    }
}

fn fresh() -> (Remote, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(1_000_000));
    (remote(ct::test_policy(), clock.clone()), clock)
}

fn assert_full<B: FullBroker>(_: &B) {}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tokens_and_grants_contract_over_ipc() {
    let (b, _) = fresh();
    assert_full(&b);
    ct::tokens::run(&fresh).await;
    ct::grants::run(&fresh).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flows_contract_over_ipc() {
    use ct::flows;
    let (b, c) = fresh();
    flows::kernel_blocks_on_l4(&b, &c).await;
    let (b, c) = fresh();
    flows::voice_destruction_asks_on_l4(&b, &c).await;
    let (b, c) = fresh();
    flows::tainted_egress_asks_on_l4(&b, &c).await;
    let (b, c) = fresh();
    flows::raise_requires_valid_proof(&b, &c).await;
    let (b, c) = fresh();
    flows::approval_flow(&b, &c).await;
}

/// Odstępstwo od `flows::agent_cannot_raise_own_level`: podniesienie przez agentkę — jak
/// w kontrakcie (`SelfEscalation`, bez prośby); obniżenie przez agentkę — także odmowa (rola
/// `Core` nie przekaże źródła „agentka”, a podanie go jako zmiany właściciela fałszowałoby Audyt
/// i kartę w Broker-UI). Obniżenie przez właściciela działa od razu.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_origin_level_change_is_refused_by_core_relay() {
    let (b, _) = fresh();
    let s1 = SessionId::new("s1");
    let target = AutonomyTarget::Session {
        session: s1.clone(),
    };
    for level in [AutonomyLevel::L4, AutonomyLevel::L1] {
        let req = AutonomyChangeRequest {
            target: target.clone(),
            level,
            until_ms: None,
            origin: ChangeOrigin::Agent("delta".into()),
        };
        assert_eq!(
            b.request_autonomy_change(req).await,
            Err(BrokerError::KernelBlock(KernelRule::SelfEscalation))
        );
    }
    assert!(b.pending().is_empty(), "żadnej prośby w Broker-UI");
    assert_eq!(b.autonomy(&s1, None), AutonomyLevel::L3);
    let owner = AutonomyChangeRequest {
        target,
        level: AutonomyLevel::L1,
        until_ms: None,
        origin: ChangeOrigin::UserInterface,
    };
    assert_eq!(b.request_autonomy_change(owner).await, Ok(None));
    assert_eq!(b.autonomy(&s1, None), AutonomyLevel::L1);
    let voice_policy = b
        .request_policy_change(ct::test_policy(), ChangeOrigin::UserVoice)
        .await;
    assert_eq!(
        voice_policy,
        Err(BrokerError::KernelBlock(KernelRule::KernelPolicyChange))
    );
}
