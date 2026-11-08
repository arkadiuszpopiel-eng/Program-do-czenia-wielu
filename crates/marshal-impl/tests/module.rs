//! Testy modułu: manifest, cykl życia, nadzór z magistrali, raport dzienny o 21:00 czasu
//! polskiego, księga reguł w pliku, brak tłumacza.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use core_bus_contract::{Event, EventBus, Level};
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};
use marshal_contract::{
    Approver, DailyReport, EVENT_ESCALATION, EVENT_REPORT, Marshal, MarshalError, event_kind,
};
use marshal_impl::{FileMarshalStore, MODULE_TOML, MarshalModule, MarshalStore, NoTranslator};

async fn settle() {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
}

#[test]
fn manifest_is_valid() {
    let m = core_registry_contract::ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "marshal");
}

#[tokio::test(start_paused = true)]
async fn bus_watch_report_and_persistence() {
    let start = 1_790_852_400_000; // 2026-10-01 11:00 UTC = 13:00 w Warszawie
    let dir = std::env::temp_dir().join(format!("alfa-marshal-{}", std::process::id()));
    let store = Arc::new(FileMarshalStore::new(dir.join("rules.json")));
    let bus = FakeBus::default();
    let mut module = MarshalModule::new(Arc::new(NoTranslator), store.clone())
        .unwrap()
        .with_start_ms(start);
    assert_eq!(module.health(), HealthStatus::NotStarted);
    assert_eq!(
        module.approve(1, Approver::UserInterface),
        Err(MarshalError::NotStarted)
    );
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    assert!(matches!(
        module.propose("cokolwiek").await,
        Err(MarshalError::Translator(_))
    ));
    // Nadzór z magistrali: zadanie przerwane siłą → eskalacja.
    for (name, payload) in [
        (
            "scheduler.task.submitted",
            serde_json::json!({"task": "a", "title": "Porządki", "at_ms": start}),
        ),
        (
            "scheduler.task.aborted",
            serde_json::json!({"task": "a", "at_ms": start}),
        ),
        (
            "scheduler.task.finished",
            serde_json::json!({"task": "a", "at_ms": start, "result": "cancelled"}),
        ),
    ] {
        bus.publish(Event::new(event_kind(name), Level::Info, payload))
            .await
            .unwrap();
    }
    settle().await;
    let esc = bus.recorded_of_kind(&event_kind(EVENT_ESCALATION));
    assert_eq!(esc.len(), 1);
    assert!(
        esc[0].payload["message"]
            .as_str()
            .unwrap()
            .contains("Porządki")
    );
    // Raport dzienny o 21:00 czasu polskiego (19:00 UTC) — 8 h później.
    tokio::time::sleep(Duration::from_millis(8 * 3_600_000 + 1_000)).await;
    settle().await;
    let reports = bus.recorded_of_kind(&event_kind(EVENT_REPORT));
    assert_eq!(reports.len(), 1);
    let report: DailyReport = serde_json::from_value(reports[0].payload.clone()).unwrap();
    assert_eq!(
        (report.submitted, report.cancelled, report.escalations),
        (1, 1, 1)
    );
    // Księga reguł w pliku.
    let p = module.propose_drafts(
        "bez mostów",
        vec![serde_json::json!({"id": "bez-mostow", "then": [{"effect": "deny_bridges"}]})],
    );
    module.approve(p.id, Approver::UserInterface).unwrap();
    assert_eq!(store.load().unwrap().unwrap().active().len(), 1);
    module.stop().await.unwrap();
    assert!(matches!(module.stop().await, Err(ModuleError::NotStarted)));
    let mut again = MarshalModule::new(Arc::new(NoTranslator), store.clone()).unwrap();
    let ctx = ModuleContext::new(again.manifest().id.clone(), Arc::new(bus.clone()));
    again.start(ctx).await.unwrap();
    assert_eq!(again.rules().len(), 1);
    assert!(again.effective().bridges_denied);
    again.stop().await.unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
