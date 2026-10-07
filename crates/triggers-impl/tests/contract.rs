//! Współdzielony test kontraktowy `triggers-contract` na implementacji (zatrzymany zegar tokio,
//! zadania do atrapy schedulera).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_fake::FakeBus;
use core_registry_contract::{Module, ModuleContext};
use scheduler_contract::{Scheduler, TaskSpec};
use scheduler_fake::FakeScheduler;
use triggers_contract::contract_tests::{self, Harness};
use triggers_impl::{MemTriggerStore, NoFileWatch, TriggersModule};

struct ImplHarness {
    module: TriggersModule,
    scheduler: Arc<FakeScheduler>,
}

async fn settle() {
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

#[async_trait]
impl Harness for ImplHarness {
    type T = TriggersModule;

    fn triggers(&self) -> &TriggersModule {
        &self.module
    }

    async fn advance(&self, ms: u64) {
        settle().await;
        tokio::time::sleep(Duration::from_millis(ms)).await;
        settle().await;
    }

    fn submitted(&self) -> Vec<TaskSpec> {
        self.scheduler.tasks().into_iter().map(|v| v.spec).collect()
    }

    fn now_ms(&self) -> u64 {
        self.module.now_ms().unwrap_or_default()
    }
}

async fn harness(start: u64) -> ImplHarness {
    let scheduler = Arc::new(FakeScheduler::starting_at(start));
    let mut module = TriggersModule::new(
        scheduler.clone(),
        Arc::new(MemTriggerStore::default()),
        Arc::new(NoFileWatch),
    )
    .unwrap()
    .with_start_ms(start);
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(FakeBus::default()));
    module.start(ctx).await.unwrap();
    settle().await;
    ImplHarness { module, scheduler }
}

#[tokio::test(start_paused = true)]
async fn contract_suite_on_impl() {
    contract_tests::run_all(harness).await;
}
