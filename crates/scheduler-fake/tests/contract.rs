//! Współdzielony test kontraktowy `scheduler-contract` na atrapie (wirtualny zegar).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use async_trait::async_trait;
use scheduler_contract::contract_tests::{self, Harness, Script};
use scheduler_contract::{Dispatch, Resource, SteerEnvelope, TaskId};
use scheduler_fake::FakeScheduler;

struct FakeHarness(FakeScheduler);

#[async_trait]
impl Harness for FakeHarness {
    type S = FakeScheduler;

    fn scheduler(&self) -> &FakeScheduler {
        &self.0
    }

    fn script(&self, task: &TaskId, script: Script) {
        self.0.script(task, script);
    }

    async fn advance(&self, ms: u64) {
        self.0.advance(ms);
    }

    fn seen_steering(&self, task: &TaskId) -> Vec<(u32, SteerEnvelope)> {
        self.0.seen_steering(task)
    }

    fn dispatches(&self) -> Vec<Dispatch> {
        self.0.dispatches()
    }

    fn held(&self) -> BTreeMap<TaskId, Vec<Resource>> {
        self.0.held()
    }

    fn now_ms(&self) -> u64 {
        self.0.now_ms()
    }

    async fn restart(&mut self) {
        self.0.restart().unwrap();
    }
}

#[tokio::test]
async fn contract_suite_on_fake() {
    contract_tests::run_all(|| async { FakeHarness(FakeScheduler::new()) }).await;
}
