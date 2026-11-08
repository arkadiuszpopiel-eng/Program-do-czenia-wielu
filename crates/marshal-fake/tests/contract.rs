//! Współdzielony test kontraktowy `marshal-contract` na atrapie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use async_trait::async_trait;
use core_bus_contract::Event;
use marshal_contract::MarshalCore;
use marshal_contract::contract_tests::{self, Harness};
use marshal_fake::{FakeMarshal, FakeMarshalHost};

struct FakeHarness(FakeMarshal);

#[async_trait]
impl Harness for FakeHarness {
    type M = MarshalCore<FakeMarshalHost>;

    fn marshal(&self) -> &Self::M {
        self.0.core()
    }

    fn script(&self, text: &str, drafts: Vec<serde_json::Value>) {
        self.0.script(text, drafts);
    }

    async fn advance(&self, ms: u64) {
        self.0.advance(ms);
    }

    fn now_ms(&self) -> u64 {
        self.0.now_ms()
    }

    fn events(&self) -> Vec<Event> {
        self.0.events()
    }
}

#[tokio::test]
async fn contract_suite_on_fake() {
    contract_tests::run_all(|start| async move { FakeHarness(FakeMarshal::new(start)) }).await;
}
