//! Testy atrapy rejestru: kontrakt współdzielony + symulacja stanów i dziennik wywołań.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use core_bus_fake::FakeBus;
use core_registry_contract::contract_tests::{self, Harness, manifest};
use core_registry_contract::{
    HealthStatus, Lifecycle, ModuleId, ModuleState, Registry, RegistryError,
};
use core_registry_fake::FakeRegistry;

fn harness() -> Harness<FakeRegistry> {
    let bus = FakeBus::default();
    let registry = FakeRegistry::new(Arc::new(bus.clone()));
    let clock = registry.clock();
    Harness {
        idle_timeout: registry.idle_timeout(),
        registry,
        bus: Arc::new(bus),
        advance: Box::new(move |d| clock.advance(d)),
    }
}

fn id(s: &str) -> ModuleId {
    ModuleId::new(s).unwrap()
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(harness).await;
}

#[tokio::test]
async fn simulated_states_and_injected_failure() {
    let fake = FakeRegistry::new(Arc::new(FakeBus::default()));
    let m = manifest("stt", Lifecycle::OnDemand, &["stt-contract@1"], &[]);
    fake.register_manifest(m).await.unwrap();
    fake.fail_next_start(&id("stt")).await.unwrap();
    assert!(matches!(
        fake.activate(&id("stt")).await,
        Err(RegistryError::StartFailed { .. })
    ));
    fake.activate(&id("stt")).await.unwrap();
    assert_eq!(
        fake.health(&id("stt")).await.unwrap(),
        HealthStatus::Healthy
    );
    fake.set_state(
        &id("stt"),
        ModuleState::Degraded {
            reason: "GPU".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        fake.health(&id("stt")).await.unwrap(),
        HealthStatus::Degraded("GPU".into())
    );
    let failed = ModuleState::Failed {
        restarts: 3,
        reason: "crash".into(),
    };
    fake.set_state(&id("stt"), failed).await.unwrap();
    assert_eq!(
        fake.health(&id("stt")).await.unwrap(),
        HealthStatus::Unhealthy("crash".into())
    );
    assert_eq!(
        fake.set_state(&id("nope"), ModuleState::Ready).await,
        Err(RegistryError::UnknownModule(id("nope")))
    );
    assert_eq!(
        fake.calls(),
        ["register:stt", "activate:stt", "activate:stt"]
    );
}

#[tokio::test]
async fn virtual_time_and_custom_timeout() {
    let fake = FakeRegistry::new(Arc::new(FakeBus::default()))
        .with_idle_timeout(Duration::from_secs(10))
        .with_external([]);
    let m = manifest(
        "x",
        Lifecycle::Lazy,
        &["x-contract@1"],
        &["core-bus-contract@1"],
    );
    fake.register_manifest(m).await.unwrap();
    assert!(matches!(
        fake.start_order().await,
        Err(RegistryError::MissingContract { .. })
    ));
    let fake =
        FakeRegistry::new(Arc::new(FakeBus::default())).with_idle_timeout(Duration::from_secs(10));
    let m = manifest(
        "x",
        Lifecycle::Lazy,
        &["x-contract@1"],
        &["core-bus-contract@1"],
    );
    fake.register_manifest(m).await.unwrap();
    fake.acquire(&"x-contract@1".parse().unwrap())
        .await
        .unwrap();
    fake.advance(Duration::from_secs(11));
    assert_eq!(fake.elapsed(), Duration::from_secs(11));
    assert_eq!(fake.unload_idle().await.unwrap(), [id("x")]);
}
