//! Współdzielony test kontraktowy `marshal-contract` na implementacji (zegar tokio zatrzymany).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::Event;
use core_bus_fake::FakeBus;
use core_registry_contract::{Module, ModuleContext};
use marshal_contract::contract_tests::{self, Harness, ScriptedTranslator};
use marshal_impl::{MarshalModule, MemMarshalStore};

struct ImplHarness {
    module: MarshalModule,
    translator: Arc<ScriptedTranslator>,
    bus: FakeBus,
}

async fn settle() {
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

#[async_trait]
impl Harness for ImplHarness {
    type M = MarshalModule;

    fn marshal(&self) -> &MarshalModule {
        &self.module
    }

    fn script(&self, text: &str, drafts: Vec<serde_json::Value>) {
        self.translator.script(text, drafts);
    }

    async fn advance(&self, ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        settle().await;
    }

    fn now_ms(&self) -> u64 {
        self.module.now_ms().unwrap_or_default()
    }

    fn events(&self) -> Vec<Event> {
        self.bus.recorded().iter().map(|e| (**e).clone()).collect()
    }
}

async fn harness(start: u64) -> ImplHarness {
    let translator = Arc::new(ScriptedTranslator::default());
    let bus = FakeBus::default();
    let mut module = MarshalModule::new(translator.clone(), Arc::new(MemMarshalStore::default()))
        .unwrap()
        .with_start_ms(start);
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    settle().await;
    ImplHarness {
        module,
        translator,
        bus,
    }
}

#[tokio::test(start_paused = true)]
async fn contract_suite_on_impl() {
    contract_tests::run_all(harness).await;
}
