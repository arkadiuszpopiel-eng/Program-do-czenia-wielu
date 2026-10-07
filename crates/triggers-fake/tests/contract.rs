//! Współdzielony test kontraktowy `triggers-contract` na atrapie (wirtualny zegar).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use async_trait::async_trait;
use scheduler_contract::TaskSpec;
use triggers_contract::TriggersCore;
use triggers_contract::contract_tests::{self, Harness};
use triggers_fake::{FakeTriggerHost, FakeTriggers};

struct FakeHarness(FakeTriggers, Arc<TriggersCore<FakeTriggerHost>>);

#[async_trait]
impl Harness for FakeHarness {
    type T = TriggersCore<FakeTriggerHost>;

    fn triggers(&self) -> &Self::T {
        &self.1
    }

    async fn advance(&self, ms: u64) {
        self.0.advance(ms);
    }

    fn submitted(&self) -> Vec<TaskSpec> {
        self.0.submitted()
    }

    fn now_ms(&self) -> u64 {
        self.0.now_ms()
    }
}

#[tokio::test]
async fn contract_suite_on_fake() {
    contract_tests::run_all(|start| async move {
        let f = FakeTriggers::new(start);
        let core = f.core();
        FakeHarness(f, core)
    })
    .await;
}
