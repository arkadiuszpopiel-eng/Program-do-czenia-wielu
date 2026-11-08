//! Współdzielony test kontraktowy na implementacji (zatrzymany zegar tokio, skryptowana
//! wykonawczyni za portem `TaskExecutor`, restart = zatrzymanie łagodne + start z tego samego
//! magazynu stanu).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_fake::FakeBus;
use core_registry_contract::{Module, ModuleContext};
use scheduler_contract::contract_tests::{self, Harness, Script, ScriptedExecutor};
use scheduler_contract::{Dispatch, MemSnapshotStore, Resource, SteerEnvelope, TaskId};
use scheduler_impl::{SchedulerModule, UnlimitedBudget};

struct ImplHarness {
    module: SchedulerModule,
    executor: ScriptedExecutor,
    store: Arc<MemSnapshotStore>,
    bus: FakeBus,
}

async fn settle() {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
}

async fn started(
    executor: ScriptedExecutor,
    store: Arc<MemSnapshotStore>,
    bus: FakeBus,
) -> SchedulerModule {
    let mut module =
        SchedulerModule::new(Arc::new(executor), store, Arc::new(UnlimitedBudget)).unwrap();
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus));
    module.start(ctx).await.unwrap();
    settle().await;
    module
}

async fn harness() -> ImplHarness {
    let executor = ScriptedExecutor::new();
    let store = Arc::new(MemSnapshotStore::new());
    let bus = FakeBus::default();
    let module = started(executor.clone(), Arc::clone(&store), bus.clone()).await;
    ImplHarness {
        module,
        executor,
        store,
        bus,
    }
}

#[async_trait]
impl Harness for ImplHarness {
    type S = SchedulerModule;

    fn scheduler(&self) -> &SchedulerModule {
        &self.module
    }

    fn script(&self, task: &TaskId, script: Script) {
        self.executor.script(task, script);
    }

    async fn advance(&self, ms: u64) {
        settle().await;
        tokio::time::sleep(Duration::from_millis(ms)).await;
        settle().await;
    }

    fn seen_steering(&self, task: &TaskId) -> Vec<(u32, SteerEnvelope)> {
        self.executor.seen(task)
    }

    fn dispatches(&self) -> Vec<Dispatch> {
        self.executor.dispatches()
    }

    fn held(&self) -> BTreeMap<TaskId, Vec<Resource>> {
        self.module
            .core()
            .map(|c| c.held_resources())
            .unwrap_or_default()
    }

    fn now_ms(&self) -> u64 {
        self.module
            .core()
            .map(|c| scheduler_contract::SchedHost::now_ms(c.host().as_ref()))
            .unwrap_or_default()
    }

    async fn restart(&mut self) {
        self.module.stop().await.unwrap();
        settle().await;
        self.module = started(
            self.executor.clone(),
            Arc::clone(&self.store),
            self.bus.clone(),
        )
        .await;
    }
}

#[tokio::test(start_paused = true)]
async fn contract_suite_on_impl() {
    contract_tests::run_all(harness).await;
}
