//! Współdzielony test kontraktowy rejestru na `ModuleRegistry` (FakeBus + zegar wirtualny).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use core_bus_fake::{FakeBus, VirtualClock};
use core_registry_contract::contract_tests::{self, Harness};
use core_registry_impl::ModuleRegistry;

fn harness() -> Harness<ModuleRegistry> {
    let clock = VirtualClock::default();
    let bus = FakeBus::new(clock.clone());
    let now = clock.clone();
    let registry =
        ModuleRegistry::new(Arc::new(bus.clone())).with_clock(Arc::new(move || now.now()));
    let idle_timeout = registry.config().idle_unload;
    Harness {
        registry,
        bus: Arc::new(bus),
        idle_timeout,
        advance: Box::new(move |d| {
            clock.advance(chrono::Duration::from_std(d).unwrap());
        }),
    }
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(harness).await;
}
