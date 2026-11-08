//! Sterownik produkcyjny: zegar (epoka + tokio `Instant`), timer terminów, wejścia z magistrali,
//! publikacja `triggers.*`, zapis stanu w tle.

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use core_bus_contract::{BusItem, Event, EventBus, EventFilter};
use futures_util::StreamExt;
use scheduler_contract::{EVENT_FINISHED, Scheduler, TaskId, TaskSpec};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use triggers_contract::{TriggerHost, TriggerInput, TriggerSnapshot, Triggers, TriggersCore};

use crate::ports::{FileWatchPort, TriggerStore};

/// Zdarzenie: nie udało się zapisać stanu wyzwalaczy.
pub const EVENT_PERSIST_FAILED: &str = "triggers.persist_failed";
/// Zdarzenie sesji: dopisano turę.
pub const SESSION_TURN_APPENDED: &str = "session.turn.appended";

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Otoczenie produkcyjne.
pub struct ImplHost {
    epoch_ms: u64,
    start: Instant,
    scheduler: Arc<dyn Scheduler>,
    events: mpsc::UnboundedSender<Vec<Event>>,
    wake: Arc<Notify>,
    pending: Mutex<Option<TriggerSnapshot>>,
    persist_wake: Arc<Notify>,
    files: Arc<dyn FileWatchPort>,
}

impl TriggerHost for ImplHost {
    fn now_ms(&self) -> u64 {
        self.epoch_ms.saturating_add(millis(self.start.elapsed()))
    }

    fn emit(&self, events: Vec<Event>) {
        let _ = self.events.send(events);
    }

    fn submit(&self, task: TaskSpec) -> Result<TaskId, String> {
        let mut ids = self
            .scheduler
            .submit(vec![task])
            .map_err(|e| e.to_string())?;
        ids.pop()
            .ok_or_else(|| "scheduler nie zwrócił zadania".into())
    }

    fn persist(&self, snapshot: &TriggerSnapshot) {
        *lock(&self.pending) = Some(snapshot.clone());
        self.persist_wake.notify_one();
    }

    fn wake(&self) {
        self.wake.notify_one();
    }

    fn watch(&self, dirs: Vec<String>) {
        self.files.watch(dirs);
    }
}

/// Uruchomione wyzwalacze.
pub(crate) struct Service {
    pub(crate) core: Arc<TriggersCore<ImplHost>>,
    store: Arc<dyn TriggerStore>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl Service {
    pub(crate) async fn start(
        bus: Arc<dyn EventBus>,
        scheduler: Arc<dyn Scheduler>,
        store: Arc<dyn TriggerStore>,
        files: Arc<dyn FileWatchPort>,
        start_ms: Option<u64>,
    ) -> Result<Arc<Self>, String> {
        let engine = match store.load()? {
            Some(s) => triggers_contract::TriggerEngine::restore(s)?,
            None => triggers_contract::TriggerEngine::new(),
        };
        let (tx, rx) = mpsc::unbounded_channel();
        let wake = Arc::new(Notify::new());
        let persist_wake = Arc::new(Notify::new());
        let epoch_ms = start_ms.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, millis)
        });
        let host = Arc::new(ImplHost {
            epoch_ms,
            start: Instant::now(),
            scheduler,
            events: tx,
            wake: Arc::clone(&wake),
            pending: Mutex::new(None),
            persist_wake: Arc::clone(&persist_wake),
            files,
        });
        let core = TriggersCore::new(host, engine);
        let filter = EventFilter {
            kind_prefixes: vec![SESSION_TURN_APPENDED.into(), EVENT_FINISHED.into()],
            ..EventFilter::all()
        };
        let stream = bus.subscribe(filter).await.map_err(|e| e.to_string())?;
        let svc = Arc::new(Self {
            core,
            store,
            tasks: Mutex::new(Vec::new()),
        });
        let weak = Arc::downgrade(&svc);
        *lock(&svc.tasks) = vec![
            tokio::spawn(publisher(bus, rx)),
            tokio::spawn(timer(weak.clone(), wake)),
            tokio::spawn(persister(weak.clone(), persist_wake)),
            tokio::spawn(listener(weak, stream)),
        ];
        Ok(svc)
    }

    pub(crate) fn shutdown(&self) -> Result<(), String> {
        for task in lock(&self.tasks).drain(..) {
            task.abort();
        }
        self.store.save(&self.core.snapshot())
    }
}

/// Zdarzenie magistrali → wejście wyzwalaczy (zadanie bez pochodzenia w ładunku jest pomijane:
/// bez niego nie da się sprawdzić łańcucha).
pub fn input_from_event(event: &Event) -> Option<TriggerInput> {
    let p = &event.payload;
    let text = |k: &str| p.get(k).and_then(|v| v.as_str()).map(str::to_owned);
    match event.kind.as_str() {
        SESSION_TURN_APPENDED => Some(TriggerInput::NewMessage {
            session: core_bus_contract::SessionId::new(text("session")?),
            turn: text("turn")?,
            role: text("role").unwrap_or_default(),
        }),
        EVENT_FINISHED => Some(TriggerInput::TaskFinished {
            task: TaskId::new(text("task")?),
            result: text("result")?,
            origin: serde_json::from_value(p.get("origin")?.clone()).ok()?,
            taint: p
                .get("taint")
                .and_then(|t| serde_json::from_value(t.clone()).ok())
                .unwrap_or_default(),
        }),
        _ => None,
    }
}

async fn listener(svc: Weak<Service>, mut stream: core_bus_contract::EventStream) {
    while let Some(item) = stream.next().await {
        let BusItem::Event(event) = item else {
            continue;
        };
        let Some(input) = input_from_event(&event) else {
            continue;
        };
        let Some(s) = svc.upgrade() else {
            return;
        };
        s.core.input(input);
    }
}

async fn timer(svc: Weak<Service>, wake: Arc<Notify>) {
    loop {
        let Some((next, now)) = svc
            .upgrade()
            .map(|s| (s.core.next_wake(), s.core.host().now_ms()))
        else {
            return;
        };
        match next {
            Some(at) if at <= now => {}
            Some(at) => {
                let sleep = tokio::time::sleep(Duration::from_millis(at - now));
                tokio::select! {
                    () = sleep => {}
                    () = wake.notified() => continue,
                }
            }
            None => {
                wake.notified().await;
                continue;
            }
        }
        let Some(s) = svc.upgrade() else {
            return;
        };
        s.core.tick();
    }
}

async fn publisher(bus: Arc<dyn EventBus>, mut rx: mpsc::UnboundedReceiver<Vec<Event>>) {
    while let Some(events) = rx.recv().await {
        for event in events {
            let _ = bus.publish(event).await;
        }
    }
}

async fn persister(svc: Weak<Service>, wake: Arc<Notify>) {
    loop {
        wake.notified().await;
        let Some(s) = svc.upgrade() else {
            return;
        };
        let Some(snapshot) = lock(&s.core.host().pending).take() else {
            continue;
        };
        let store = Arc::clone(&s.store);
        drop(s);
        // Błąd zapisu: następna zmiana spróbuje ponownie; stan w pamięci pozostaje aktualny.
        let error = match tokio::task::spawn_blocking(move || store.save(&snapshot)).await {
            Ok(Ok(())) => continue,
            Ok(Err(e)) => e,
            Err(e) => e.to_string(),
        };
        if let Some(s) = svc.upgrade() {
            s.core.host().emit(vec![Event::new(
                triggers_contract::event_kind(EVENT_PERSIST_FAILED),
                core_bus_contract::Level::Warn,
                serde_json::json!({ "error": error }),
            )]);
        }
    }
}
