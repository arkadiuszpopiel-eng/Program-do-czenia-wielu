//! Atrapa `triggers` (SPEC „Fake”): ten sam deterministyczny rdzeń co `-impl`
//! ([`TriggersCore`]), ale na **wirtualnym zegarze** (czas płynie tylko przez
//! [`FakeTriggers::advance`]); zadania nagrywane zamiast schedulera (z możliwością odmowy),
//! zdarzenia nagrywane zamiast magistrali, stan w pamięci (restart = nowy rdzeń ze stanu).
//! Do testów UI, Kreatora agentów, Marszałka — wyłącznie jako dev-dependency.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::Event;
use scheduler_contract::{TaskId, TaskSpec};
use triggers_contract::{TriggerEngine, TriggerHost, TriggerSnapshot, TriggersCore};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Otoczenie atrapy.
#[derive(Default)]
pub struct FakeTriggerHost {
    clock: AtomicU64,
    events: Mutex<Vec<Event>>,
    submitted: Mutex<Vec<TaskSpec>>,
    reject: AtomicBool,
    snapshot: Mutex<Option<TriggerSnapshot>>,
    watched: Mutex<Vec<String>>,
}

impl TriggerHost for FakeTriggerHost {
    fn now_ms(&self) -> u64 {
        self.clock.load(Ordering::SeqCst)
    }

    fn emit(&self, events: Vec<Event>) {
        lock(&self.events).extend(events);
    }

    fn submit(&self, task: TaskSpec) -> Result<TaskId, String> {
        if self.reject.load(Ordering::SeqCst) {
            return Err("scheduler odrzucił zadanie (atrapa)".into());
        }
        let id = task.id.clone();
        lock(&self.submitted).push(task);
        Ok(id)
    }

    fn persist(&self, snapshot: &TriggerSnapshot) {
        *lock(&self.snapshot) = Some(snapshot.clone());
    }

    fn watch(&self, dirs: Vec<String>) {
        *lock(&self.watched) = dirs;
    }
}

/// Wyzwalacze-atrapa.
pub struct FakeTriggers {
    host: Arc<FakeTriggerHost>,
    core: Mutex<Arc<TriggersCore<FakeTriggerHost>>>,
}

impl FakeTriggers {
    /// Nowa atrapa z zegarem ustawionym na `start_ms` (ms UTC).
    pub fn new(start_ms: u64) -> Self {
        let host = Arc::new(FakeTriggerHost::default());
        host.clock.store(start_ms, Ordering::SeqCst);
        Self {
            core: Mutex::new(TriggersCore::new(Arc::clone(&host), TriggerEngine::new())),
            host,
        }
    }

    /// Rdzeń (implementuje `Triggers`).
    pub fn core(&self) -> Arc<TriggersCore<FakeTriggerHost>> {
        Arc::clone(&lock(&self.core))
    }

    /// Bieżący czas (ms UTC).
    pub fn now_ms(&self) -> u64 {
        self.host.now_ms()
    }

    /// Przesuwa czas o `ms`, wyzwalając po kolei każdy termin po drodze.
    pub fn advance(&self, ms: u64) {
        let target = self.now_ms().saturating_add(ms);
        let mut guard = 0u32;
        while let Some(t) = self.core().next_wake().filter(|t| *t <= target) {
            self.host
                .clock
                .store(t.max(self.now_ms()), Ordering::SeqCst);
            self.core().tick();
            guard += 1;
            if guard > 1_000_000 {
                break;
            }
        }
        self.host.clock.store(target, Ordering::SeqCst);
        self.core().tick();
    }

    /// Przesuwa zegar bez wyzwalania (program wyłączony / uśpiony) — test zaległości.
    pub fn jump(&self, ms: u64) {
        self.host
            .clock
            .store(self.now_ms().saturating_add(ms), Ordering::SeqCst);
    }

    /// Zadania przekazane do schedulera.
    pub fn submitted(&self) -> Vec<TaskSpec> {
        lock(&self.host.submitted).clone()
    }

    /// Nagrane zdarzenia.
    pub fn events(&self) -> Vec<Event> {
        lock(&self.host.events).clone()
    }

    /// Obserwowane katalogi (port obserwacji plików).
    pub fn watched(&self) -> Vec<String> {
        lock(&self.host.watched).clone()
    }

    /// Scheduler odrzuca (`true`) albo przyjmuje zadania.
    pub fn set_reject(&self, reject: bool) {
        self.host.reject.store(reject, Ordering::SeqCst);
    }

    /// Restart: nowy rdzeń z ostatnio zapisanego stanu.
    pub fn restart(&self) -> Result<(), String> {
        let engine = match lock(&self.host.snapshot).clone() {
            Some(s) => TriggerEngine::restore(s)?,
            None => TriggerEngine::new(),
        };
        *lock(&self.core) = TriggersCore::new(Arc::clone(&self.host), engine);
        Ok(())
    }
}
