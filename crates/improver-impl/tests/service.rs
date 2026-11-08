//! Usługa: moduł rejestru i zdarzenia na magistrali, trwała kolejka, cykl bezczynności
//! z prawdziwą bramką holdoutu (`evals-fake`) — Ulepszacz widzi tylko wynik zbiorczy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use evals_contract::contract_tests::{build_suite, cases};
use evals_contract::{
    CandidateRunner, CaseOutcome, EvalCase, GatePolicy, ManualClock, Split, SuiteStatus, Variant,
};
use evals_fake::{FakeCatalog, FakeGate};
use improver_contract::contract_tests::{ScriptedGate, ScriptedProposer, TestVerifier};
use improver_contract::{
    CandidateSet, ChangeTarget, EVENT_DEPLOYED, Improver, ImproverPolicy, MetricsSnapshot,
    RunConditions, Stage,
};
use improver_impl::{ImproverService, MODULE_TOML};
use serde_json::json;

const IDLE: RunConditions = RunConditions {
    on_battery: false,
    game_mode: false,
    user_idle: true,
};

fn top_k(value: i64) -> CandidateSet {
    CandidateSet {
        title: "mniej wspomnień w kontekście".into(),
        rationale: "test".into(),
        source: "test".into(),
        targets: vec![ChangeTarget::Config {
            key: "memory.recall.top_k".into(),
            value: json!(value),
        }],
    }
}

fn snapshot(pass_rate: f64) -> MetricsSnapshot {
    MetricsSnapshot {
        ts_ms: 0,
        metrics: BTreeMap::from([("pass_rate".to_owned(), pass_rate)]),
        observations: Vec::new(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn module_lifecycle_and_bus_events() {
    let store = common::store(&[("memory.recall.top_k".into(), json!(8))]).await;
    let mut service = ImproverService::new(
        store,
        Arc::new(ScriptedGate::default()),
        Arc::new(TestVerifier),
        ImproverPolicy::default(),
        Arc::new(ManualClock::new(5)),
        None,
    )
    .unwrap();
    let m = service.manifest();
    assert_eq!(
        (m.id.as_str(), m.lifecycle),
        ("improver", Lifecycle::OnDemand)
    );
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert!(m.capabilities.is_empty());
    assert!(MODULE_TOML.contains("evals-contract@1"));
    assert_eq!(service.health(), HealthStatus::NotStarted);
    assert_eq!(service.stop().await, Err(ModuleError::NotStarted));
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(service.manifest().id.clone(), Arc::new(bus.clone()));
    service.start(ctx.clone()).await.unwrap();
    assert_eq!(
        service.start(ctx.clone()).await,
        Err(ModuleError::AlreadyStarted)
    );
    assert_eq!(service.health(), HealthStatus::Healthy);
    let p = service.submit(top_k(4)).await.unwrap();
    let p = service.evaluate(p.id).await.unwrap();
    assert_eq!(p.stage, Stage::Deployed { auto: true });
    let kind = improver_contract::improver_event(
        EVENT_DEPLOYED,
        core_bus_contract::Level::Info,
        json!(null),
    )
    .kind;
    let mut waited = 0;
    while bus.recorded_of_kind(&kind).is_empty() && waited < 200 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        waited += 1;
    }
    assert_eq!(bus.recorded_of_kind(&kind).len(), 1);
    service.stop().await.unwrap();
    service.start(ctx).await.unwrap();
    service.stop().await.unwrap();
}

#[tokio::test]
async fn queue_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("improver/proposals.json");
    let store = common::store(&[("memory.recall.top_k".into(), json!(8))]).await;
    let make = || {
        ImproverService::new(
            store.clone(),
            Arc::new(ScriptedGate::default()),
            Arc::new(TestVerifier),
            ImproverPolicy::default(),
            Arc::new(ManualClock::new(5)),
            Some(path.clone()),
        )
        .unwrap()
    };
    let first = make();
    let wide = first.submit(top_k(12)).await.unwrap();
    let wide = first.evaluate(wide.id).await.unwrap();
    assert_eq!(wide.stage, Stage::AwaitingApproval);
    drop(first);
    let second = make();
    let restored = second.proposals();
    assert_eq!(restored, vec![wide.clone()]);
    let next = second.submit(top_k(4)).await.unwrap();
    assert!(next.id > wide.id);
    std::fs::write(&path, b"{zepsute").unwrap();
    assert!(
        ImproverService::new(
            store,
            Arc::new(ScriptedGate::default()),
            Arc::new(TestVerifier),
            ImproverPolicy::default(),
            Arc::new(ManualClock::new(5)),
            Some(path),
        )
        .is_err()
    );
}

/// System w piaskownicy: przypadek zaliczony, gdy `memory.recall.top_k` ≤ 6.
struct TopKRunner;

#[async_trait]
impl CandidateRunner for TopKRunner {
    async fn run(
        &self,
        variant: &Variant,
        case: &EvalCase,
        repeat: u32,
    ) -> Result<CaseOutcome, String> {
        let k = variant
            .patch
            .get("memory.recall.top_k")
            .and_then(|v| v.as_f64())
            .unwrap_or(99.0);
        Ok(CaseOutcome::new(case, repeat, k <= 6.0))
    }
}

#[tokio::test]
async fn idle_cycle_with_real_holdout_gate() {
    let mut public = cases("dev", Split::Dev, 5, "jawne");
    public.extend(cases("test", Split::Test, 12, "jawne"));
    let public = build_suite(
        "f8-improver-r0",
        "F8/r0",
        SuiteStatus::Proposed,
        &public,
        Vec::new(),
    );
    let holdout = build_suite(
        "f8-improver-r0",
        "r0",
        SuiteStatus::Frozen,
        &cases("UKRYTE", Split::Holdout, 15, "UKRYTE"),
        Vec::new(),
    );
    let catalog = Arc::new(FakeCatalog::new());
    catalog.add_suite(public.manifest, public.files).unwrap();
    let gate = FakeGate::new(catalog, Arc::new(TopKRunner), GatePolicy::default())
        .with_holdout(holdout.manifest, holdout.files)
        .unwrap();
    let store = common::store(&[("memory.recall.top_k".into(), json!(8))]).await;
    let service = ImproverService::with_proposers(
        store.clone(),
        Arc::new(gate),
        Arc::new(TestVerifier),
        ImproverPolicy::default(),
        Arc::new(ManualClock::new(5)),
        None,
        vec![Arc::new(ScriptedProposer::new(vec![top_k(4)]))],
    )
    .unwrap();
    assert!(
        service
            .cycle(
                &snapshot(0.9),
                RunConditions {
                    on_battery: true,
                    ..IDLE
                }
            )
            .await
            .is_err()
    );
    let report = service.cycle(&snapshot(0.9), IDLE).await.unwrap();
    assert_eq!(
        (report.created.len(), report.auto_deployed.len()),
        (1, 1),
        "{report:?}"
    );
    let p = &service.proposals()[0];
    let holdout_verdict = p.holdout.as_ref().unwrap();
    assert!(
        holdout_verdict.passed() && holdout_verdict.n_cases == 15 && holdout_verdict.repeats >= 5
    );
    let text = serde_json::to_string(&service.proposals()).unwrap();
    assert!(!text.contains("UKRYTE"), "propozycja ujawnia holdout");
    assert_eq!(
        store.history().last().map(|r| r.origin.clone()),
        Some(core_config_contract::Origin::Improver)
    );
    // Regresja po wdrożeniu → rollback w kolejnym cyklu (bez nowej propozycji — wychładzanie).
    let report = service.cycle(&snapshot(0.5), IDLE).await.unwrap();
    assert_eq!(report.rolled_back.len(), 1);
    assert!(report.created.is_empty());
    assert!(service.recent_events().len() >= 4);
}
