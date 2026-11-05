#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use approval_judge_contract::{DenyCategory, JudgeTurn};
use approval_judge_fake::ScriptedJudge;
use async_trait::async_trait;
use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::{AutonomyLevel, Destructiveness, Reversibility, RuleId};
use safety_broker_contract::{
    ActionRequest, ApprovalId, ApprovalStatus, AttenuateRequest, AutonomyChangeRequest,
    BrokerMetrics, CapToken, Capability, ChangeOrigin, Decision, Holder, KernelPolicy,
    PlanDecision, PlanRequest, SessionSecurity, TaintSource, TokenId,
};

use super::*;

fn unsupported<T>() -> Result<T, BrokerError> {
    Err(BrokerError::InvalidRequest("atrapa".into()))
}

/// Atrapa Brokera: kolejka sędziego i zapisane werdykty; reszta nieobsługiwana.
#[derive(Default)]
struct StubBroker {
    queue: Mutex<Vec<JudgeRequest>>,
    resolved: Mutex<Vec<(ApprovalId, JudgeResolution)>>,
}

#[async_trait]
impl Broker for StubBroker {
    async fn decide(&self, _: ActionRequest) -> Result<Decision, BrokerError> {
        unsupported()
    }
    fn verify(&self, _: &CapToken, _: &Capability, _: &Holder) -> Result<(), BrokerError> {
        unsupported()
    }
    async fn attenuate(
        &self,
        _: &CapToken,
        _: &Holder,
        _: AttenuateRequest,
    ) -> Result<CapToken, BrokerError> {
        unsupported()
    }
    async fn revoke(&self, _: TokenId) -> Result<usize, BrokerError> {
        unsupported()
    }
    async fn revoke_holder(&self, _: &Holder) -> Result<usize, BrokerError> {
        unsupported()
    }
    async fn report_untrusted_input(
        &self,
        _: &SessionId,
        _: TaintSource,
    ) -> Result<(), BrokerError> {
        unsupported()
    }
    fn session_security(&self, _: &SessionId) -> SessionSecurity {
        SessionSecurity::default()
    }
    async fn submit_plan(&self, _: PlanRequest) -> Result<PlanDecision, BrokerError> {
        unsupported()
    }
    fn approval_status(&self, _: ApprovalId, _: &Holder) -> Result<ApprovalStatus, BrokerError> {
        unsupported()
    }
    async fn request_autonomy_change(
        &self,
        _: AutonomyChangeRequest,
    ) -> Result<Option<ApprovalId>, BrokerError> {
        unsupported()
    }
    fn autonomy(&self, _: &SessionId, _: Option<&AgentId>) -> AutonomyLevel {
        AutonomyLevel::L3
    }
    async fn request_policy_change(
        &self,
        _: KernelPolicy,
        _: ChangeOrigin,
    ) -> Result<ApprovalId, BrokerError> {
        unsupported()
    }
    fn metrics(&self) -> BrokerMetrics {
        BrokerMetrics::default()
    }
    fn judge_queue(&self) -> Vec<JudgeRequest> {
        self.queue.lock().unwrap().clone()
    }
    async fn judge_resolve(
        &self,
        id: ApprovalId,
        resolution: JudgeResolution,
    ) -> Result<(), BrokerError> {
        self.queue.lock().unwrap().retain(|r| r.approval != id);
        self.resolved.lock().unwrap().push((id, resolution));
        Ok(())
    }
}

struct Ctx(Option<JudgeContext>);

impl JudgeContextSource for Ctx {
    fn context(&self, _: &str, _: Option<&str>) -> Option<JudgeContext> {
        self.0.clone()
    }
}

fn request(id: u64) -> JudgeRequest {
    JudgeRequest {
        approval: ApprovalId(id),
        holder: Holder::agent("s1", "delta"),
        tool: "net.fetch".into(),
        capability: Capability::NetEgress(
            safety_broker_contract::HostPattern::parse("api.nbp.pl").unwrap(),
        ),
        summary: Some("pobranie kursu".into()),
        command: None,
        workdir: Some(r"E:\Projekt".into()),
        reversible: Reversibility::Yes,
        destructive: Destructiveness::None,
        rules: vec![RuleId::EgressNotAllowlisted],
        tainted: false,
        outside_workdir: false,
        host: Some("api.nbp.pl".into()),
        model_location: Some(safety_broker_contract::ModelLocation::Local),
        deadline_ms: 0,
    }
}

fn ctx(local_only: bool) -> JudgeContext {
    JudgeContext {
        goal: "kurs EUR".into(),
        transcript: vec![JudgeTurn::Owner {
            text: "Pobierz kurs EUR".into(),
        }],
        limits: vec!["nie wysyłaj nic".into()],
        task: Some("run-7".into()),
        local_only,
    }
}

fn allow() -> JudgeVerdict {
    JudgeVerdict::Allow {
        reason: "ok".into(),
    }
}

#[test]
fn input_carries_action_and_context() {
    let input = build_input(&request(1), ctx(true));
    assert_eq!(input.goal, "kurs EUR");
    assert_eq!(input.action.host.as_deref(), Some("api.nbp.pl"));
    assert!(input.action.capability_summary.contains(r"E:\Projekt"));
    assert_eq!(input.limits, ["nie wysyłaj nic"]);
}

#[test]
fn local_run_never_uses_cloud_judge() {
    let cloud: Arc<dyn ApprovalJudge> = Arc::new(ScriptedJudge::new());
    let only_cloud = Judges {
        local: None,
        any: Some(cloud),
    };
    assert!(only_cloud.pick(true).is_none());
    assert!(only_cloud.pick(false).is_some());
}

#[tokio::test]
async fn verdict_goes_back_with_task_and_model() {
    let broker = Arc::new(StubBroker::default());
    broker.queue.lock().unwrap().push(request(4));
    let local = Arc::new(ScriptedJudge::with_queue([allow()]));
    let driver = JudgeDriver::new(
        broker.clone(),
        Judges {
            local: Some(local.clone()),
            any: None,
        },
        Arc::new(Ctx(Some(ctx(true)))),
    );
    driver.judge_one(request(4)).await.unwrap();
    let resolved = broker.resolved.lock().unwrap().clone();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].0, ApprovalId(4));
    assert_eq!(resolved[0].1.verdict, allow());
    assert_eq!(resolved[0].1.task.as_deref(), Some("run-7"));
    assert_eq!(resolved[0].1.model, "fake:scripted");
    assert_eq!(local.seen().len(), 1);
}

#[tokio::test]
async fn missing_context_or_model_escalates() {
    for (judges, context) in [
        (
            Judges {
                local: Some(
                    Arc::new(ScriptedJudge::with_queue([allow()])) as Arc<dyn ApprovalJudge>
                ),
                any: None,
            },
            None,
        ),
        (Judges::default(), Some(ctx(true))),
    ] {
        let broker = Arc::new(StubBroker::default());
        let driver = JudgeDriver::new(broker.clone(), judges, Arc::new(Ctx(context)));
        driver.judge_one(request(1)).await.unwrap();
        let resolved = broker.resolved.lock().unwrap().clone();
        assert!(
            matches!(resolved[0].1.verdict, JudgeVerdict::Escalate { .. }),
            "{resolved:?}"
        );
    }
}

#[tokio::test]
async fn tick_judges_each_request_once() {
    let broker = Arc::new(StubBroker::default());
    broker.queue.lock().unwrap().push(request(9));
    let judge = Arc::new(ScriptedJudge::new());
    judge.set_default(JudgeVerdict::Deny {
        category: DenyCategory::Other,
        reason: "nie".into(),
    });
    let driver = JudgeDriver::new(
        broker.clone(),
        Judges {
            local: Some(judge.clone()),
            any: None,
        },
        Arc::new(Ctx(Some(ctx(true)))),
    );
    assert_eq!(driver.tick(), 1);
    for _ in 0..50 {
        if !broker.resolved.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(broker.resolved.lock().unwrap().len(), 1);
    assert_eq!(driver.tick(), 0, "kolejka pusta po werdykcie");
    assert_eq!(judge.seen().len(), 1);
}

#[derive(Default)]
struct MemLog(Mutex<Vec<JudgeLogEntry>>);

impl JudgeLog for MemLog {
    fn record(&self, entry: JudgeLogEntry) {
        self.0.lock().unwrap().push(entry);
    }
}

/// Dziennik werdyktów: wpis z sesją, narzędziem i informacją, czy Sędzia Cieni przyjął werdykt.
#[tokio::test]
async fn verdict_is_logged_with_acceptance() {
    let broker = Arc::new(StubBroker::default());
    let log = Arc::new(MemLog::default());
    let driver = JudgeDriver::with_log(
        broker.clone(),
        Judges {
            local: Some(Arc::new(ScriptedJudge::with_queue([allow()]))),
            any: None,
        },
        Arc::new(Ctx(Some(ctx(true)))),
        log.clone(),
    );
    driver.judge_one(request(5)).await.unwrap();
    let entries = log.0.lock().unwrap().clone();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].verdict, allow());
    assert_eq!(entries[0].tool, request(5).tool);
    assert!(entries[0].accepted);
}

/// P2: przebieg lokalny (fakty narzędzia) — sędzia w chmurze nie dostaje zapisu, choć historia mówi „chmura”.
#[tokio::test]
async fn local_run_location_overrides_cloud_history() {
    let broker = Arc::new(StubBroker::default());
    let cloud = Arc::new(ScriptedJudge::with_queue([allow()]));
    let driver = JudgeDriver::new(
        broker.clone(),
        Judges {
            local: None,
            any: Some(cloud.clone()),
        },
        Arc::new(Ctx(Some(ctx(false)))),
    );
    driver.judge_one(request(6)).await.unwrap();
    assert!(cloud.seen().is_empty());
    let resolved = broker.resolved.lock().unwrap().clone();
    assert!(matches!(
        resolved[0].1.verdict,
        JudgeVerdict::Escalate { .. }
    ));
}
