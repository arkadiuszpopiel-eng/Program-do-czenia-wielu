//! Testy modułu: manifest, cykl życia, wejścia z magistrali (wiadomość, koniec zadania z
//! pochodzeniem i taintem), zdarzenia `triggers.*`, stan w pliku i restart.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use core_bus_contract::{Event, EventBus, Level};
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};
use safety_broker_contract::TaintSource;
use scheduler_contract::{EVENT_FINISHED, Scheduler, TaskOrigin, event_kind};
use scheduler_fake::FakeScheduler;
use triggers_contract::contract_tests::{user_trigger, utc};
use triggers_contract::{
    Actor, CronExpr, EVENT_CREATED, EVENT_FIRED, FinishFilter, TriggerError, TriggerInput,
    TriggerKind, Triggers,
};
use triggers_impl::{
    FileTriggerStore, FileWatchPort, MODULE_TOML, MemTriggerStore, NoFileWatch,
    SESSION_TURN_APPENDED, TriggerStore, TriggersModule, input_from_event,
};

#[derive(Default)]
struct Watch(Mutex<Vec<String>>);

impl FileWatchPort for Watch {
    fn watch(&self, dirs: Vec<String>) {
        *self.0.lock().unwrap() = dirs;
    }
}

async fn settle() {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
}

#[test]
fn manifest_is_valid() {
    let m = core_registry_contract::ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "triggers");
}

#[test]
fn bus_events_become_inputs() {
    let msg = Event::new(
        event_kind(SESSION_TURN_APPENDED),
        Level::Info,
        serde_json::json!({"session": "s", "turn": "t", "role": "user"}),
    );
    assert!(matches!(
        input_from_event(&msg),
        Some(TriggerInput::NewMessage { .. })
    ));
    let done = Event::new(
        event_kind(EVENT_FINISHED),
        Level::Info,
        serde_json::json!({"task": "a", "result": "failed",
            "origin": {"origin": "trigger", "trigger_id": "x", "depth": 2}, "taint": ["web"]}),
    );
    let Some(TriggerInput::TaskFinished { origin, taint, .. }) = input_from_event(&done) else {
        panic!("brak wejścia");
    };
    assert_eq!(origin.trigger_depth(), 2);
    assert_eq!(taint, vec![TaintSource::Web]);
    let no_origin = Event::new(
        event_kind(EVENT_FINISHED),
        Level::Info,
        serde_json::json!({"task": "a", "result": "failed"}),
    );
    assert!(
        input_from_event(&no_origin).is_none(),
        "bez pochodzenia — pomijane"
    );
}

#[tokio::test(start_paused = true)]
async fn lifecycle_bus_chain_and_file_restart() {
    let start = utc("2026-06-01 10:00");
    let scheduler = Arc::new(FakeScheduler::starting_at(start));
    let dir = std::env::temp_dir().join(format!("alfa-triggers-{}", std::process::id()));
    let store = Arc::new(FileTriggerStore::new(dir.join("state.json")));
    let watch = Arc::new(Watch::default());
    let bus = FakeBus::default();
    let mut module = TriggersModule::new(scheduler.clone(), store.clone(), watch.clone())
        .unwrap()
        .with_start_ms(start);
    assert_eq!(module.health(), HealthStatus::NotStarted);
    assert_eq!(
        module.create(user_trigger("x", TriggerKind::Manual), Actor::User),
        Err(TriggerError::NotStarted)
    );
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    assert_eq!(module.health(), HealthStatus::Healthy);
    // Łańcuch: wyzwalacz „po zadaniu” reaguje na zakończenie zadania z magistrali.
    let after = user_trigger(
        "po-raporcie",
        TriggerKind::TaskFinished {
            task_prefix: Some("raport".into()),
            outcome: FinishFilter::Any,
        },
    );
    module.create(after, Actor::User).unwrap();
    let pdf = user_trigger(
        "pdf",
        TriggerKind::FileInDir {
            dir: "C:\\Pobrane".into(),
            pattern: None,
        },
    );
    module.create(pdf, Actor::User).unwrap();
    assert_eq!(
        watch.0.lock().unwrap().clone(),
        vec!["C:\\Pobrane".to_owned()]
    );
    bus.publish(Event::new(
        event_kind(EVENT_FINISHED),
        Level::Info,
        serde_json::json!({"task": "raport/7", "result": "succeeded", "origin": {"origin": "user"}}),
    ))
    .await
    .unwrap();
    settle().await;
    let tasks = scheduler.tasks();
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].spec.origin,
        TaskOrigin::Trigger {
            trigger_id: "po-raporcie".into(),
            depth: 1
        }
    );
    assert_eq!(bus.recorded_of_kind(&event_kind(EVENT_CREATED)).len(), 2);
    assert_eq!(bus.recorded_of_kind(&event_kind(EVENT_FIRED)).len(), 1);
    // Cron co godzinę, potem restart ze stanu w pliku: wyzwalacze i dziennik przetrwają.
    let hourly = user_trigger(
        "co-godzine",
        TriggerKind::Cron {
            expr: CronExpr::parse("0 * * * *").unwrap(),
        },
    );
    module.create(hourly, Actor::User).unwrap();
    tokio::time::sleep(Duration::from_millis(3_600_000)).await;
    settle().await;
    assert_eq!(scheduler.tasks().len(), 2);
    module.stop().await.unwrap();
    assert!(matches!(module.stop().await, Err(ModuleError::NotStarted)));
    assert_eq!(store.load().unwrap().unwrap().len(), 3);
    let mut again = TriggersModule::new(scheduler.clone(), store.clone(), Arc::new(NoFileWatch))
        .unwrap()
        .with_start_ms(utc("2026-06-01 11:00"));
    let ctx = ModuleContext::new(again.manifest().id.clone(), Arc::new(bus.clone()));
    again.start(ctx).await.unwrap();
    assert_eq!(again.list().len(), 3);
    assert_eq!(again.log(None, 100).len(), 2);
    tokio::time::sleep(Duration::from_millis(3_600_000)).await;
    settle().await;
    assert_eq!(scheduler.tasks().len(), 3);
    again.stop().await.unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let mem = MemTriggerStore::default();
    assert!(mem.load().unwrap().is_none());
}
