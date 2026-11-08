//! „Zdrowie systemu" na prawdziwych modułach (Diagnosta, Ulepszacz, evale) nad atrapą
//! konfiguracji i rejestrem w procesie: wstrzyknięta awaria (sygnał `diagnostics.symptom`) →
//! incydent z propozycją → zgoda w panelu → naprawa zweryfikowana → „Cofnij" przywraca konfigurację
//! 1:1; zdarzenie `HealthChanged` dla UI; Ulepszacz bez modelu i bez replayu niczego nie wdraża.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use app_api::EventHub;
use app_api::dto::{AlfaEvent, HealthView};
use app_health::{HealthApp, HealthDeps};
use core_bus_contract::{Event, EventBus, EventKind, Level};
use core_bus_impl::{BroadcastBus, BusConfig};
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope};
use core_config_fake::FakeConfigStore;
use core_registry_impl::ModuleRegistry;

const ROUTE_KEY: &str = "router.routes.anthropic.enabled";

struct World {
    _dir: tempfile::TempDir,
    app: Arc<HealthApp>,
    bus: Arc<dyn EventBus>,
    config: Arc<FakeConfigStore>,
    events: EventHub,
}

async fn world(autonomy: &str) -> World {
    let dir = tempfile::tempdir().unwrap();
    let bus: Arc<dyn EventBus> = Arc::new(BroadcastBus::new(BusConfig::default()));
    let config = Arc::new(FakeConfigStore::new(MachineId::new("test-maszyna")));
    config
        .set(
            &ConfigKey::new("diagnostician.autonomy").unwrap(),
            Some(autonomy.into()),
            &Scope::Global,
            &ConfigLayer::Shared,
            Origin::User,
        )
        .await
        .unwrap();
    let registry = Arc::new(ModuleRegistry::new(bus.clone()));
    let events = EventHub::start(Duration::from_millis(5));
    let app = HealthApp::open(HealthDeps {
        local: dir.path().to_path_buf(),
        config: config.clone(),
        registry,
        bus: bus.clone(),
        events: events.clone(),
        kernel_roots: vec![dir.path().join("broker-dev")],
        evals_root: Some(dir.path().join("evals-public")),
        runner: None,
        scan_every_ms: Some(20),
    })
    .await;
    app.spawn_bridge(bus.clone()).await;
    World {
        _dir: dir,
        app,
        bus,
        config,
        events,
    }
}

async fn route_value(config: &FakeConfigStore) -> Option<serde_json::Value> {
    config
        .get(&ConfigKey::new(ROUTE_KEY).unwrap(), &Scope::Global)
        .await
        .unwrap()
}

async fn until(app: &HealthApp, check: impl Fn(&HealthView) -> bool) -> HealthView {
    for _ in 0..300 {
        let v = app.report().await.unwrap();
        if check(&v) {
            return v;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!(
        "raport nie osiągnął stanu: {:#?}",
        app.report().await.unwrap()
    );
}

async fn inject_revoked_key(bus: &Arc<dyn EventBus>) {
    let payload = serde_json::json!({
        "module": "providers-api",
        "symptom": {"code": "http", "status": 401},
        "target": "anthropic",
    });
    bus.publish(Event::new(
        EventKind::Custom("diagnostics.symptom".into()),
        Level::Warn,
        payload,
    ))
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn injected_failure_incident_repair_and_undo_through_commands() {
    let w = world("propose_only").await;
    let mut rx = w.events.subscribe();
    assert!(w.app.health("diagnostician") == core_registry_contract::HealthStatus::Healthy);
    inject_revoked_key(&w.bus).await;
    let view = until(&w.app, |v| !v.pending.is_empty()).await;
    let card = &view.pending[0];
    assert!(!card.diff.is_empty(), "propozycja z diffem");
    assert!(!card.rollback_plan.is_empty(), "plan cofnięcia");
    assert!(!card.kernel);
    assert!(view.incidents.iter().any(|i| i.id == card.id));
    let repaired = w.app.approve(card.id).await.unwrap();
    assert!(
        repaired
            .repaired
            .iter()
            .any(|r| r.id == card.id && r.undoable)
    );
    assert_eq!(route_value(&w.config).await, Some(serde_json::json!(false)));
    let undone = w.app.undo(card.id).await.unwrap();
    assert!(
        !undone
            .repaired
            .iter()
            .any(|r| r.id == card.id && r.undoable)
    );
    assert_eq!(
        route_value(&w.config).await,
        None,
        "konfiguracja 1:1 sprzed naprawy"
    );
    assert!(
        w.app.undo(card.id).await.is_err(),
        "drugie cofnięcie odrzucone"
    );
    tokio::time::sleep(Duration::from_millis(30)).await;
    let mut seen = false;
    while let Ok(batch) = rx.try_recv() {
        seen |= batch
            .iter()
            .any(|e| matches!(e, AlfaEvent::HealthChanged { .. }));
    }
    assert!(seen, "HealthChanged dla UI");
}

#[tokio::test(flavor = "multi_thread")]
async fn rejected_proposal_changes_nothing() {
    let w = world("propose_only").await;
    inject_revoked_key(&w.bus).await;
    let view = until(&w.app, |v| !v.pending.is_empty()).await;
    let id = view.pending[0].id;
    let after = w.app.reject(id).await.unwrap();
    assert!(after.pending.iter().all(|p| p.id != id));
    assert_eq!(route_value(&w.config).await, None);
    assert!(w.app.approve(id).await.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn improver_without_model_and_replay_deploys_nothing() {
    let w = world("auto_low_risk").await;
    let before = w.app.improver_view().unwrap();
    assert!(before.proposals.is_empty());
    assert!(!before.idle_cycle, "bez portu bezczynności — tylko ręcznie");
    let after = w.app.cycle().await;
    let view = match after {
        Ok(v) => v,
        Err(_) => w.app.improver_view().unwrap(),
    };
    assert!(
        view.proposals
            .iter()
            .all(|p| !p.stage.starts_with("deployed")),
        "nic nie wdrożone bez oceny"
    );
    assert!(w.app.improver_approve(999, "x").await.is_err());
    let evals = w.app.evals_view();
    assert!(evals.available, "{:?}", evals.reason);
    assert!(evals.suites.is_empty());
    assert_eq!(evals.holdout_suites, 0);
    assert!(w.app.verify("nie-ma").is_err());
}
