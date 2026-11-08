//! Testy specyficzne dla implementacji: manifest, crash-loop, health, zdarzenia, reaper.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::EventKind;
use core_bus_fake::{FakeBus, VirtualClock};
use core_registry_contract::contract_tests::{StartLog, StubModule, manifest};
use core_registry_contract::{
    EVENT_HEALTH, EVENT_RESOLVE_FAILED, EVENT_STATE_CHANGED, HealthStatus, Lifecycle, Module,
    ModuleContext, ModuleError, ModuleId, ModuleManifest, ModuleState, Registry, RegistryError,
    registry_event_kind,
};
use core_registry_impl::{ModuleRegistry, RegistryConfig, spawn_idle_reaper};

fn id(s: &str) -> ModuleId {
    ModuleId::new(s).unwrap()
}

fn setup(config: RegistryConfig) -> (ModuleRegistry, FakeBus, VirtualClock) {
    let clock = VirtualClock::default();
    let bus = FakeBus::new(clock.clone());
    let now = clock.clone();
    let registry = ModuleRegistry::new(Arc::new(bus.clone()))
        .with_config(config)
        .with_clock(Arc::new(move || now.now()));
    (registry, bus, clock)
}

async fn state(r: &ModuleRegistry, m: &str) -> ModuleState {
    r.list()
        .await
        .into_iter()
        .find(|s| s.id.as_str() == m)
        .unwrap()
        .state
}

/// Moduł z przełączanym stanem zdrowia i opcjonalną awarią `stop`.
struct Probe {
    manifest: ModuleManifest,
    health: Arc<Mutex<HealthStatus>>,
    fail_stop: bool,
    running: bool,
}

#[async_trait]
impl Module for Probe {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    async fn start(&mut self, _ctx: ModuleContext) -> Result<(), ModuleError> {
        self.running = true;
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), ModuleError> {
        if self.fail_stop {
            return Err(ModuleError::Other("nie da się".into()));
        }
        self.running = false;
        Ok(())
    }
    fn health(&self) -> HealthStatus {
        self.health.lock().unwrap().clone()
    }
}

fn probe(m: &str, fail_stop: bool) -> (Box<Probe>, Arc<Mutex<HealthStatus>>) {
    let health = Arc::new(Mutex::new(HealthStatus::Healthy));
    let module = Probe {
        manifest: manifest(m, Lifecycle::OnDemand, &[], &[]),
        health: health.clone(),
        fail_stop,
        running: false,
    };
    (Box::new(module), health)
}

#[test]
fn module_toml_is_valid() {
    let m = core_registry_impl::manifest().unwrap();
    assert_eq!(m.id.as_str(), "core-registry");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.lifecycle, Lifecycle::Always);
    assert_eq!(m.provides[0].to_string(), "core-registry-contract@1");
    assert_eq!(m.requires[0].to_string(), "core-bus-contract@1");
}

#[tokio::test]
async fn crash_loop_is_limited_and_reset_by_reenable() {
    let config = RegistryConfig {
        crash_loop_limit: 2,
        ..RegistryConfig::default()
    };
    let (r, _, _) = setup(config);
    let log = StartLog::default();
    let m = manifest("f", Lifecycle::OnDemand, &[], &[]);
    r.register(Box::new(StubModule::failing(m, log.clone())))
        .await
        .unwrap();
    for _ in 0..2 {
        let err = r.activate(&id("f")).await.unwrap_err();
        assert!(matches!(err, RegistryError::StartFailed { .. }));
    }
    let err = r.activate(&id("f")).await.unwrap_err();
    assert_eq!(
        err,
        RegistryError::CrashLoop {
            module: id("f"),
            restarts: 2
        }
    );
    assert_eq!(log.lock().unwrap().len(), 2, "trzecia próba nie woła start");
    assert!(matches!(
        state(&r, "f").await,
        ModuleState::Failed { restarts: 2, .. }
    ));
    r.set_enabled(&id("f"), false).await.unwrap();
    r.set_enabled(&id("f"), true).await.unwrap();
    assert!(matches!(
        r.activate(&id("f")).await,
        Err(RegistryError::StartFailed { .. })
    ));
}

#[tokio::test]
async fn health_drives_degraded_state_and_publishes() {
    let (r, bus, _) = setup(RegistryConfig::default());
    let (module, health) = probe("h", false);
    r.register(module).await.unwrap();
    r.activate(&id("h")).await.unwrap();
    *health.lock().unwrap() = HealthStatus::Degraded("wolno".into());
    assert_eq!(
        r.health(&id("h")).await.unwrap(),
        HealthStatus::Degraded("wolno".into())
    );
    assert_eq!(
        state(&r, "h").await,
        ModuleState::Degraded {
            reason: "wolno".into()
        }
    );
    *health.lock().unwrap() = HealthStatus::Unhealthy("padł".into());
    r.health(&id("h")).await.unwrap();
    assert!(matches!(state(&r, "h").await, ModuleState::Degraded { .. }));
    *health.lock().unwrap() = HealthStatus::Healthy;
    r.health(&id("h")).await.unwrap();
    assert_eq!(state(&r, "h").await, ModuleState::Ready);
    let events = bus.recorded_of_kind(&registry_event_kind(EVENT_HEALTH));
    let statuses: Vec<_> = events.iter().map(|e| e.payload["status"].clone()).collect();
    assert_eq!(statuses, ["degraded", "unhealthy", "healthy"]);
    let changes = bus.recorded_of_kind(&registry_event_kind(EVENT_STATE_CHANGED));
    let degraded = changes
        .iter()
        .find(|e| e.payload["to"] == "degraded")
        .unwrap();
    assert_eq!(degraded.payload["reason"], "wolno");
}

#[tokio::test]
async fn stop_failure_marks_failed() {
    let (r, _, _) = setup(RegistryConfig::default());
    let (module, _) = probe("s", true);
    r.register(module).await.unwrap();
    r.activate(&id("s")).await.unwrap();
    assert!(matches!(
        r.deactivate(&id("s")).await,
        Err(RegistryError::StopFailed { .. })
    ));
    assert!(matches!(state(&r, "s").await, ModuleState::Failed { .. }));
    r.activate(&id("s")).await.unwrap();
    assert!(
        r.shutdown().await.is_err(),
        "błąd zatrzymania jest zgłaszany"
    );
}

#[tokio::test]
async fn resolve_failures_are_published() {
    let (r, bus, _) = setup(RegistryConfig::default());
    let log = StartLog::default();
    let on_demand = manifest("c", Lifecycle::OnDemand, &["c-contract@1"], &[]);
    r.register(Box::new(StubModule::new(on_demand, log)))
        .await
        .unwrap();
    assert!(r.acquire(&"c-contract@1".parse().unwrap()).await.is_err());
    assert!(r.acquire(&"x-contract@1".parse().unwrap()).await.is_err());
    let events = bus.recorded_of_kind(&EventKind::Custom(EVENT_RESOLVE_FAILED.into()));
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].payload["contract"], "x-contract@1");
}

#[tokio::test]
async fn kernel_contracts_are_external_and_always_dependents_block_deactivate() {
    let (r, _, _) = setup(RegistryConfig::default());
    let log = StartLog::default();
    let k = manifest(
        "k",
        Lifecycle::Always,
        &[],
        &["core-bus-contract@1", "y-contract@1"],
    );
    let y = manifest("y", Lifecycle::Lazy, &["y-contract@1"], &[]);
    r.register(Box::new(StubModule::new(k, log.clone())))
        .await
        .unwrap();
    r.register(Box::new(StubModule::new(y, log.clone())))
        .await
        .unwrap();
    assert_eq!(r.boot().await.unwrap(), [id("y"), id("k")]);
    assert_eq!(
        r.deactivate(&id("y")).await,
        Err(RegistryError::InUse {
            module: id("y"),
            dependents: vec![id("k")]
        })
    );
    assert_eq!(r.shutdown().await.unwrap(), [id("k"), id("y")]);
}

#[tokio::test]
async fn failed_dependency_blocks_dependent_start() {
    let (r, _, _) = setup(RegistryConfig::default());
    let log = StartLog::default();
    let a = manifest("a", Lifecycle::Always, &[], &["b-contract@1"]);
    let b = manifest("b", Lifecycle::Lazy, &["b-contract@1"], &[]);
    r.register(Box::new(StubModule::new(a, log.clone())))
        .await
        .unwrap();
    r.register(Box::new(StubModule::failing(b, log.clone())))
        .await
        .unwrap();
    assert!(matches!(
        r.boot().await,
        Err(RegistryError::StartFailed { .. })
    ));
    assert_eq!(state(&r, "a").await, ModuleState::Unloaded);
    assert_eq!(*log.lock().unwrap(), ["fail:b"]);
}

#[tokio::test]
async fn per_module_idle_override() {
    let mut config = RegistryConfig::default();
    config
        .idle_overrides
        .insert(id("fast"), Duration::from_secs(5));
    let (r, _, clock) = setup(config);
    let log = StartLog::default();
    for m in ["fast", "slow"] {
        let man = manifest(m, Lifecycle::OnDemand, &[], &[]);
        r.register(Box::new(StubModule::new(man, log.clone())))
            .await
            .unwrap();
        r.activate(&id(m)).await.unwrap();
    }
    clock.advance(chrono::Duration::seconds(6));
    assert_eq!(r.unload_idle().await.unwrap(), [id("fast")]);
}

#[tokio::test(start_paused = true)]
async fn reaper_unloads_idle_modules_periodically() {
    let (r, _, clock) = setup(RegistryConfig::default());
    let r = Arc::new(r);
    let log = StartLog::default();
    let lazy = manifest("l", Lifecycle::Lazy, &["l-contract@1"], &[]);
    r.register(Box::new(StubModule::new(lazy, log.clone())))
        .await
        .unwrap();
    r.acquire(&"l-contract@1".parse().unwrap()).await.unwrap();
    let reaper = spawn_idle_reaper(Arc::clone(&r), Duration::from_secs(30));
    tokio::time::sleep(Duration::from_secs(31)).await;
    assert_eq!(state(&r, "l").await, ModuleState::Ready);
    clock.advance(chrono::Duration::minutes(11));
    tokio::time::sleep(Duration::from_secs(31)).await;
    assert_eq!(state(&r, "l").await, ModuleState::Unloaded);
    reaper.abort();
}
