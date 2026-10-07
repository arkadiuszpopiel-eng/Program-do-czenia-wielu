//! Moduł: zbieranie sygnałów z magistrali i cykliczny skan po `start`, zdarzenia na magistrali.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use core_bus_contract::{Event, EventBus, EventKind, Level};
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use diagnostician_contract::{
    Diagnostician, EVENT_REPAIRED, EVENT_SYMPTOM, RepairPolicy, RepairStatus,
};
use diagnostician_fake::{ChaosBroker, ChaosWorld};
use diagnostician_impl::MODULE_TOML;
use serde_json::json;
use watchdog_contract::ManualClock;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collects_signals_from_bus_and_repairs() {
    let clock = Arc::new(ManualClock::new(1_700_000_000_000));
    let world = ChaosWorld::baseline(clock.clone());
    world.mutate(|st| {
        st.gpu_ok = false;
        st.config.insert("voice.stt.device".into(), json!("vulkan"));
    });
    let svc = common::service(
        world.clone(),
        world.clone(),
        Arc::new(ChaosBroker(world.clone())),
        RepairPolicy::default(),
        clock,
        None,
    )
    .await;
    let mut svc = svc.with_scan_interval(20);
    let m = svc.manifest();
    assert_eq!(
        (m.id.as_str(), m.lifecycle),
        ("diagnostician", Lifecycle::Always)
    );
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert!(MODULE_TOML.contains("watchdog-contract@1"));
    assert_eq!(svc.health(), HealthStatus::NotStarted);
    assert_eq!(svc.stop().await, Err(ModuleError::NotStarted));
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(svc.manifest().id.clone(), Arc::new(bus.clone()));
    svc.start(ctx.clone()).await.unwrap();
    assert_eq!(
        svc.start(ctx.clone()).await,
        Err(ModuleError::AlreadyStarted)
    );
    assert_eq!(svc.health(), HealthStatus::Healthy);
    let payload = json!({"module": "voice-stt", "symptom": {"code": "gpu_device_lost"}, "target": "voice-stt", "details": {"device_key": "voice.stt.device"}});
    bus.publish(Event::new(
        EventKind::Custom(EVENT_SYMPTOM.into()),
        Level::Error,
        payload,
    ))
    .await
    .unwrap();
    let mut waited = 0;
    while !svc
        .repairs()
        .iter()
        .any(|r| r.status == RepairStatus::Verified)
        && waited < 300
    {
        tokio::time::sleep(Duration::from_millis(10)).await;
        waited += 1;
    }
    assert_eq!(world.snapshot().config["voice.stt.device"], json!("cpu"));
    let kind = EventKind::Custom(EVENT_REPAIRED.into());
    let mut waited = 0;
    while bus.recorded_of_kind(&kind).is_empty() && waited < 300 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        waited += 1;
    }
    assert_eq!(bus.recorded_of_kind(&kind).len(), 1);
    svc.stop().await.unwrap();
    svc.start(ctx).await.unwrap();
    svc.stop().await.unwrap();
    assert!(!svc.recent_events().is_empty());
}
