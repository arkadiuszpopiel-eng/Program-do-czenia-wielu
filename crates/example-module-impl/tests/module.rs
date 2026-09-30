//! Testy implementacji: kontrakt współdzielony, manifest, cykl życia, zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use example_module_contract::{contract_tests, echo_called_kind, Echo, EchoError};
use example_module_impl::{EchoModule, MODULE_TOML};

async fn started() -> (EchoModule, FakeBus) {
    let bus = FakeBus::default();
    let mut module = EchoModule::new().unwrap();
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    (module, bus)
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|| async { started().await.0 }).await;
}

#[test]
fn manifest_is_valid_and_matches_crate() {
    let module = EchoModule::new().unwrap();
    let m = module.manifest();
    assert_eq!(m.id.as_str(), "example-module");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.lifecycle, Lifecycle::Lazy);
    assert_eq!(m.provides[0].to_string(), "example-module-contract@1");
    assert!(MODULE_TOML.contains("core-bus-contract@1"));
}

#[tokio::test]
async fn lifecycle_and_health() {
    let mut module = EchoModule::new().unwrap();
    assert_eq!(module.health(), HealthStatus::NotStarted);
    assert_eq!(module.echo("x").await, Err(EchoError::NotStarted));
    assert_eq!(module.stop().await, Err(ModuleError::NotStarted));

    let bus = FakeBus::default();
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus));
    module.start(ctx.clone()).await.unwrap();
    assert_eq!(module.health(), HealthStatus::Healthy);
    assert_eq!(module.start(ctx).await, Err(ModuleError::AlreadyStarted));
    module.stop().await.unwrap();
    assert_eq!(module.health(), HealthStatus::NotStarted);
}

#[tokio::test]
async fn echo_publishes_event_on_bus() {
    let (module, bus) = started().await;
    module.echo("hej").await.unwrap();
    module.echo("").await.unwrap_err();
    module.echo("ho").await.unwrap();
    let events = bus.recorded_of_kind(&echo_called_kind());
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].payload["seq"], serde_json::json!(1));
    assert_eq!(events[1].payload["chars"], serde_json::json!(2));
    assert_eq!(module.calls(), 2);
}
