//! Sterownik wokół [`TriggerEngine`] z otoczeniem ([`TriggerHost`]: zegar, ujście zadań,
//! zdarzenia, trwałość, obserwacja katalogów) i trait [`Triggers`] — wspólny dla `-impl`
//! i `-fake`.

use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::Event;
use scheduler_contract::{TaskId, TaskSpec};
use serde_json::json;

use crate::engine::{TaskSink, TriggerEngine, TriggerSnapshot};
use crate::events::{
    EVENT_CREATED, EVENT_REMOVED, EVENT_TOGGLED, EVENT_UPDATED, change_event, run_event,
};
use crate::record::{RunRecord, TriggerInput, TriggerView};
use crate::spec::{Actor, TriggerId, TriggerSpec};
use crate::validate::TriggerError;

/// Otoczenie wyzwalaczy.
pub trait TriggerHost: Send + Sync + 'static {
    /// Czas ścienny (ms UTC).
    fn now_ms(&self) -> u64;
    /// Zdarzenia do opublikowania.
    fn emit(&self, events: Vec<Event>);
    /// Zgłasza zadanie do schedulera.
    fn submit(&self, task: TaskSpec) -> Result<TaskId, String>;
    /// Trwały zapis stanu.
    fn persist(&self, _snapshot: &TriggerSnapshot) {}
    /// Zmienił się najbliższy termin.
    fn wake(&self) {}
    /// Zmienił się zbiór obserwowanych katalogów (port obserwacji plików platformy).
    fn watch(&self, _dirs: Vec<String>) {}
}

struct HostSink<'a, H: TriggerHost>(&'a H);

impl<H: TriggerHost> TaskSink for HostSink<'_, H> {
    fn submit(&self, task: TaskSpec) -> Result<TaskId, String> {
        self.0.submit(task)
    }
}

/// Wyzwalacze (API dla UI, Kreatora agentów, Marszałka).
pub trait Triggers: Send + Sync {
    /// Tworzy wyzwalacz (właściciel = `actor`).
    fn create(&self, spec: TriggerSpec, actor: Actor) -> Result<TriggerView, TriggerError>;
    /// Zmienia wyzwalacz.
    fn update(&self, spec: TriggerSpec, actor: Actor) -> Result<TriggerView, TriggerError>;
    /// Usuwa wyzwalacz.
    fn remove(&self, id: &TriggerId, actor: Actor) -> Result<(), TriggerError>;
    /// Włącza/wyłącza.
    fn set_enabled(&self, id: &TriggerId, enabled: bool, actor: Actor) -> Result<(), TriggerError>;
    /// „Uruchom teraz”.
    fn fire_now(&self, id: &TriggerId, actor: Actor) -> Result<RunRecord, TriggerError>;
    /// Wejście zdarzeniowe (plik, wiadomość, koniec zadania).
    fn input(&self, input: TriggerInput) -> Vec<RunRecord>;
    /// Globalne „Nie przeszkadzać”.
    fn set_dnd(&self, on: bool);
    /// Wszystkie wyzwalacze.
    fn list(&self) -> Vec<TriggerView>;
    /// Wyzwalacz.
    fn get(&self, id: &TriggerId) -> Option<TriggerView>;
    /// Dziennik uruchomień (najnowsze na końcu, najwyżej `limit`).
    fn log(&self, id: Option<&TriggerId>, limit: usize) -> Vec<RunRecord>;
}

/// Rdzeń z otoczeniem.
pub struct TriggersCore<H: TriggerHost> {
    host: Arc<H>,
    engine: Mutex<TriggerEngine>,
}

impl<H: TriggerHost> TriggersCore<H> {
    /// Nowy rdzeń (pusty albo ze stanu).
    pub fn new(host: Arc<H>, engine: TriggerEngine) -> Arc<Self> {
        let core = Arc::new(Self {
            host,
            engine: Mutex::new(engine),
        });
        let dirs = core.lock().watched_dirs();
        core.host.watch(dirs);
        core
    }

    /// Otoczenie.
    pub fn host(&self) -> &Arc<H> {
        &self.host
    }

    fn lock(&self) -> MutexGuard<'_, TriggerEngine> {
        self.engine.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Operacja zmieniająca stan: zdarzenia, zapis, budzenie, katalogi — po zwolnieniu blokady.
    fn mutate<T>(
        &self,
        f: impl FnOnce(&mut TriggerEngine, u64, &HostSink<'_, H>) -> (T, Vec<Event>),
    ) -> T {
        let now = self.host.now_ms();
        let sink = HostSink(self.host.as_ref());
        let (value, events, snapshot, dirs) = {
            let mut engine = self.lock();
            let before = engine.watched_dirs();
            let (value, events) = f(&mut engine, now, &sink);
            let dirs = engine.watched_dirs();
            (
                value,
                events,
                engine.snapshot(),
                (dirs != before).then_some(dirs),
            )
        };
        if !events.is_empty() {
            self.host.emit(events);
        }
        self.host.persist(&snapshot);
        if let Some(dirs) = dirs {
            self.host.watch(dirs);
        }
        self.host.wake();
        value
    }

    /// Upływ czasu (sterownik po `next_wake`).
    pub fn tick(&self) -> Vec<RunRecord> {
        self.mutate(|e, now, sink| {
            let records = e.tick(now, sink);
            let events = records.iter().map(run_event).collect();
            (records, events)
        })
    }

    /// Najbliższy termin.
    pub fn next_wake(&self) -> Option<u64> {
        self.lock().next_wake()
    }

    /// Stan do zapisu.
    pub fn snapshot(&self) -> TriggerSnapshot {
        self.lock().snapshot()
    }

    /// Obserwowane katalogi.
    pub fn watched_dirs(&self) -> Vec<String> {
        self.lock().watched_dirs()
    }
}

impl<H: TriggerHost> Triggers for TriggersCore<H> {
    fn create(&self, spec: TriggerSpec, actor: Actor) -> Result<TriggerView, TriggerError> {
        self.mutate(|e, now, _| match e.add(spec, &actor, now) {
            Ok(view) => {
                let ev = change_event(
                    EVENT_CREATED,
                    &view.spec.id,
                    &actor,
                    now,
                    json!({ "kind": view.spec.kind.name(), "allow_bridges": view.spec.allow_bridges }),
                );
                (Ok(view), vec![ev])
            }
            Err(err) => (Err(err), Vec::new()),
        })
    }

    fn update(&self, spec: TriggerSpec, actor: Actor) -> Result<TriggerView, TriggerError> {
        self.mutate(|e, now, _| match e.update(spec, &actor, now) {
            Ok(view) => {
                let ev = change_event(EVENT_UPDATED, &view.spec.id, &actor, now, json!({}));
                (Ok(view), vec![ev])
            }
            Err(err) => (Err(err), Vec::new()),
        })
    }

    fn remove(&self, id: &TriggerId, actor: Actor) -> Result<(), TriggerError> {
        self.mutate(|e, now, _| match e.remove(id, &actor) {
            Ok(()) => (
                Ok(()),
                vec![change_event(EVENT_REMOVED, id, &actor, now, json!({}))],
            ),
            Err(err) => (Err(err), Vec::new()),
        })
    }

    fn set_enabled(&self, id: &TriggerId, enabled: bool, actor: Actor) -> Result<(), TriggerError> {
        self.mutate(|e, now, _| match e.set_enabled(id, enabled, &actor, now) {
            Ok(()) => {
                let ev = change_event(
                    EVENT_TOGGLED,
                    id,
                    &actor,
                    now,
                    json!({ "enabled": enabled }),
                );
                (Ok(()), vec![ev])
            }
            Err(err) => (Err(err), Vec::new()),
        })
    }

    fn fire_now(&self, id: &TriggerId, actor: Actor) -> Result<RunRecord, TriggerError> {
        self.mutate(|e, now, sink| match e.fire_manual(id, &actor, now, sink) {
            Ok(record) => {
                let ev = run_event(&record);
                (Ok(record), vec![ev])
            }
            Err(err) => (Err(err), Vec::new()),
        })
    }

    fn input(&self, input: TriggerInput) -> Vec<RunRecord> {
        self.mutate(|e, now, sink| {
            let records = e.on_input(&input, now, sink);
            let events = records.iter().map(run_event).collect();
            (records, events)
        })
    }

    fn set_dnd(&self, on: bool) {
        self.mutate(|e, now, _| {
            e.set_dnd(on, now);
            ((), Vec::new())
        });
    }

    fn list(&self) -> Vec<TriggerView> {
        self.lock().list()
    }

    fn get(&self, id: &TriggerId) -> Option<TriggerView> {
        self.lock().view(id)
    }

    fn log(&self, id: Option<&TriggerId>, limit: usize) -> Vec<RunRecord> {
        self.lock().log(id, limit)
    }
}
