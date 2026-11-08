//! Testy modułu: manifest, cykl życia, zdarzenia na magistrali, stan w pliku (restart =
//! wznowienie), budżet tła z `cost-meter`, awaria wykonawczyni, `wait`, kill-switch.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};
use cost_meter_contract::{BudgetDecision, BudgetOrigin, CostMeter, LimitMode, MonthlyLimit};
use cost_meter_fake::FakeCostMeter;
use scheduler_contract::contract_tests::{Script, ScriptedExecutor};
use scheduler_contract::{
    Assignee, Dispatch, EVENT_DISPATCHED, EVENT_FINISHED, EVENT_SUBMITTED, Scheduler,
    SchedulerLite, SnapshotStore, StepGate, TaskClass, TaskError, TaskExecutor, TaskOrigin,
    TaskSpec, TaskState, Termination, WorkerResult, event_kind,
};
use scheduler_impl::{
    BackgroundBudget, CostMeterBudget, FileSnapshotStore, MODULE_TOML, SchedulerModule,
    UnlimitedBudget,
};

fn spec(id: &str) -> TaskSpec {
    TaskSpec::new(
        id,
        id,
        Assignee::AnyAgent,
        TaskClass::User,
        TaskOrigin::User,
    )
}

async fn settle() {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
}

async fn start(module: &mut SchedulerModule, bus: &FakeBus) {
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    settle().await;
}

#[test]
fn manifest_is_valid() {
    let m = core_registry_contract::ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "scheduler");
    assert!(
        m.provides
            .iter()
            .any(|p| p.to_string().starts_with("scheduler-lite-contract"))
    );
}

#[tokio::test(start_paused = true)]
async fn lifecycle_events_wait_and_kill_switch() {
    let executor = ScriptedExecutor::new();
    executor.script(&"a".into(), Script::ok(3, 100));
    let mut module = SchedulerModule::in_memory(Arc::new(executor.clone())).unwrap();
    assert_eq!(module.health(), HealthStatus::NotStarted);
    assert_eq!(module.submit(vec![spec("a")]), Err(TaskError::NotStarted));
    assert_eq!(module.kill_all(), 0);
    let bus = FakeBus::default();
    start(&mut module, &bus).await;
    assert_eq!(module.health(), HealthStatus::Healthy);
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    assert!(matches!(
        module.start(ctx).await,
        Err(ModuleError::AlreadyStarted)
    ));
    module.submit(vec![spec("a")]).unwrap();
    let t = module.wait(&"a".into()).await.unwrap();
    assert!(t.is_success());
    settle().await;
    for name in [EVENT_SUBMITTED, EVENT_DISPATCHED, EVENT_FINISHED] {
        assert_eq!(bus.recorded_of_kind(&event_kind(name)).len(), 1, "{name}");
    }
    executor.script(&"b".into(), Script::ok(100, 100));
    module.submit(vec![spec("b")]).unwrap();
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(module.kill_all() >= 1);
    settle().await;
    assert!(matches!(
        module.task(&"b".into()).unwrap().state,
        TaskState::Done {
            termination: Termination::Cancelled { .. }
        }
    ));
    module.stop().await.unwrap();
    assert!(matches!(module.stop().await, Err(ModuleError::NotStarted)));
}

#[tokio::test(start_paused = true)]
async fn file_store_resumes_after_restart() {
    let dir = std::env::temp_dir().join(format!("alfa-scheduler-{}", std::process::id()));
    let path = dir.join("state.json");
    let _ = std::fs::remove_file(&path);
    let store = Arc::new(FileSnapshotStore::new(&path));
    let executor = ScriptedExecutor::new();
    executor.script(&"dlugie".into(), Script::ok(5, 100));
    let bus = FakeBus::default();
    let mut module = SchedulerModule::new(
        Arc::new(executor.clone()),
        store.clone(),
        Arc::new(UnlimitedBudget),
    )
    .unwrap();
    start(&mut module, &bus).await;
    module.submit(vec![spec("dlugie")]).unwrap();
    tokio::time::sleep(Duration::from_millis(250)).await;
    settle().await;
    module.stop().await.unwrap();
    assert_eq!(store.load().unwrap().unwrap().active_count(), 1);
    let mut again = SchedulerModule::new(
        Arc::new(executor.clone()),
        store.clone(),
        Arc::new(UnlimitedBudget),
    )
    .unwrap();
    start(&mut again, &bus).await;
    assert!(again.wait(&"dlugie".into()).await.unwrap().is_success());
    let resumed: Vec<Dispatch> = executor
        .dispatches()
        .into_iter()
        .filter(|d| d.interrupted)
        .collect();
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].resume_from_step, 2);
    again.stop().await.unwrap();
    std::fs::write(&path, b"{nie json").unwrap();
    assert!(store.load().is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn background_budget_follows_cost_meter() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let meter = Arc::new(FakeCostMeter::new(40_000, day));
    let budget = CostMeterBudget::with_today(meter.clone(), || {
        NaiveDate::from_ymd_opt(2026, 10, 1).unwrap_or_default()
    });
    // Domyślnie budżet tła = 0 PLN (tylko modele lokalne).
    assert!(matches!(budget.check(1_000), BudgetDecision::Block { .. }));
    let mut config = meter.budget();
    config.background = MonthlyLimit::pln(10, LimitMode::Enforced);
    meter.set_budget(config, BudgetOrigin::User).await.unwrap();
    assert_eq!(budget.check(1_000), BudgetDecision::Allow);
    assert_eq!(UnlimitedBudget.check(u64::MAX), BudgetDecision::Allow);
}

struct Panicking;

#[async_trait]
impl TaskExecutor for Panicking {
    async fn execute(&self, _dispatch: Dispatch, _gate: Arc<dyn StepGate>) -> WorkerResult {
        panic!("awaria wykonawczyni (test)");
    }
}

#[tokio::test(start_paused = true)]
async fn worker_panic_is_a_failure_not_a_hang() {
    let mut module = SchedulerModule::in_memory(Arc::new(Panicking)).unwrap();
    start(&mut module, &FakeBus::default()).await;
    module.submit(vec![spec("awaria")]).unwrap();
    let t = module.wait(&"awaria".into()).await.unwrap();
    assert!(
        matches!(t, Termination::Failed { attempts: 1, ref error } if error.contains("awarii"))
    );
    module.stop().await.unwrap();
}
