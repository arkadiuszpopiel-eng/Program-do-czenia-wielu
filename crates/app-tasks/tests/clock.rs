//! Wyzwalacz czasowy na wirtualnym zegarze (tokio `start_paused`): cron „0 8 * * *" w strefie
//! Europe/Warsaw (z DST) odpala dokładnie o 08:00 (nie wcześniej), następny termin — nazajutrz i zgłasza w schedulerze zadanie
//! z pochodzeniem wyzwalacza (nigdy „użytkownik”); wyzwalacz zdarzeniowy albo ręczny z mostem CLI jest odrzucany.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_api::EventHub;
use app_api::dto::{TriggerDraft, TriggerKindView};
use app_tasks::{
    MarshalModule, RosterCtl, SchedulerModule, TasksApp, TasksParts, TriggersModule, map,
};
use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_bus_impl::{BroadcastBus, BusConfig};
use core_registry_contract::{Module, ModuleContext};
use personas_contract::{Cast, PersonaId, RoleId};
use scheduler_contract::{Dispatch, StepGate, TaskExecutor, TaskOrigin, TaskOutput, WorkerResult};
use scheduler_impl::{FileSnapshotStore, UnlimitedBudget};
use triggers_contract::{LocalTime, Tz};
use triggers_impl::{MemTriggerStore, NoFileWatch};

#[derive(Default)]
struct Recorder {
    seen: Mutex<Vec<(String, TaskOrigin)>>,
}

#[async_trait]
impl TaskExecutor for Recorder {
    async fn execute(&self, d: Dispatch, _gate: Arc<dyn StepGate>) -> WorkerResult {
        self.seen
            .lock()
            .unwrap()
            .push((d.spec.title.clone(), d.spec.origin.clone()));
        WorkerResult::Succeeded {
            output: TaskOutput::text("ok"),
        }
    }
}

async fn start<T: Module>(mut module: T, bus: &Arc<dyn EventBus>) -> Arc<T> {
    let id = module.manifest().id.clone();
    module
        .start(ModuleContext::new(id, bus.clone()))
        .await
        .unwrap();
    Arc::new(module)
}

fn draft(name: &str, kind: TriggerKindView, bridge: Option<&str>) -> TriggerDraft {
    TriggerDraft {
        name: name.into(),
        kind,
        title: name.into(),
        goal: "Przegląd poczty".into(),
        agent: None,
        bridge: bridge.map(str::to_owned),
        respect_dnd: false,
    }
}

#[tokio::test(start_paused = true)]
async fn cron_trigger_fires_at_eight_warsaw_on_virtual_clock() {
    let bus: Arc<dyn EventBus> = Arc::new(BroadcastBus::new(BusConfig::default()));
    let dir = tempfile::tempdir().unwrap();
    let recorder = Arc::new(Recorder::default());
    let roster = Arc::new(RosterCtl::new(
        Cast::solo(PersonaId::alfa(), [RoleId::conductor()], false),
        2,
    ));
    let scheduler = SchedulerModule::new(
        recorder.clone(),
        Arc::new(FileSnapshotStore::new(dir.path().join("scheduler.json"))),
        Arc::new(UnlimitedBudget),
    )
    .unwrap();
    roster.apply(&scheduler);
    let scheduler = start(scheduler, &bus).await;
    // Jutro 07:59 w Warszawie (z DST): scheduler sprawdza terminy zegarem systemowym, więc
    // wirtualny zegar wyzwalaczy startuje w przyszłości względem niego.
    let tz = Tz::warsaw();
    let today = tz
        .to_local(chrono::Utc::now().timestamp_millis())
        .unwrap()
        .date();
    let local = today.succ_opt().unwrap().and_hms_opt(7, 59, 0).unwrap();
    let LocalTime::Single(start_ms) = tz.from_local(local) else {
        panic!("07:59 zawsze jednoznaczne");
    };
    let eight = u64::try_from(start_ms + 60_000).unwrap();
    let triggers = TriggersModule::new(
        scheduler.clone(),
        Arc::new(MemTriggerStore::default()),
        Arc::new(NoFileWatch),
    )
    .unwrap()
    .with_start_ms(u64::try_from(start_ms).unwrap());
    let triggers = start(triggers, &bus).await;
    let app = TasksApp::new(TasksParts {
        scheduler,
        triggers: Some(triggers),
        marshal: None::<Arc<MarshalModule>>,
        translator: false,
        watch: false,
        bridges_denied: Arc::default(),
        roster,
        events: EventHub::start(Duration::from_millis(5)),
    });

    let morning = app
        .trigger_create(&draft(
            "Poranek",
            TriggerKindView::Cron {
                expr: "0 8 * * *".into(),
            },
            None,
        ))
        .unwrap();
    assert_eq!(morning.next_fire_at, Some(map::iso_ms(eight)));

    tokio::time::sleep(Duration::from_secs(30)).await;
    assert!(recorder.seen.lock().unwrap().is_empty(), "za wcześnie");
    tokio::time::sleep(Duration::from_secs(40)).await;
    for _ in 0..100 {
        if !recorder.seen.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let seen = recorder.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].0, "Poranek");
    assert!(
        matches!(
            seen[0].1,
            TaskOrigin::Trigger { .. } | TaskOrigin::Schedule { .. }
        ),
        "{:?}",
        seen[0].1
    );
    let log = app.trigger_log(Some(&morning.id));
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].outcome, "submitted");
    let after = app
        .triggers_list()
        .into_iter()
        .find(|t| t.id == morning.id)
        .unwrap();
    let tomorrow = local
        .date()
        .succ_opt()
        .unwrap()
        .and_hms_opt(8, 0, 0)
        .unwrap();
    let LocalTime::Single(next_ms) = tz.from_local(tomorrow) else {
        panic!("08:00 zawsze jednoznaczne");
    };
    let next_ms = u64::try_from(next_ms).unwrap();
    assert_eq!(after.next_fire_at, Some(map::iso_ms(next_ms)));

    // Most CLI: tylko harmonogram użytkownika — zdarzenie i ręczny wyzwalacz są odrzucane.
    let file = TriggerKindView::FileInDir {
        dir: "C:\\Pobrane".into(),
        pattern: None,
    };
    assert!(
        app.trigger_create(&draft("Plik", file, Some("claude_code")))
            .is_err()
    );
    assert!(
        app.trigger_create(&draft("Ręczny", TriggerKindView::Manual, Some("codex")))
            .is_err()
    );
    let nightly = TriggerKindView::Cron {
        expr: "0 3 * * *".into(),
    };
    assert!(
        app.trigger_create(&draft("Noc", nightly, Some("claude_code")))
            .is_ok()
    );
}
