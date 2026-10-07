//! Implementacja `triggers` (docs/modules/triggers/SPEC.md, PLAN §9.5, §1.3 pkt 4).
//!
//! Rdzeń (`TriggersCore`, reguły zgodności, cron/DST) pochodzi z kontraktu; ten crate dostarcza:
//! zegar ścienny i timer terminów (tokio), wejścia z magistrali (`session.turn.appended`,
//! `scheduler.task.finished` — z pochodzeniem i taintem z ładunku), wejście obserwatora plików
//! ([`TriggersModule::file_created`] + port [`FileWatchPort`]), zgłaszanie zadań do
//! schedulera (`Scheduler::submit` — pochodzenie `Trigger`, most CLI odmawia), publikację
//! `triggers.*` i stan w pliku ([`FileTriggerStore`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod ports;
mod service;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use scheduler_contract::Scheduler;
use triggers_contract::{
    Actor, RunRecord, TriggerError, TriggerId, TriggerInput, TriggerSpec, TriggerView, Triggers,
};

pub use ports::{FileTriggerStore, FileWatchPort, MemTriggerStore, NoFileWatch, TriggerStore};
pub use service::{EVENT_PERSIST_FAILED, ImplHost, SESSION_TURN_APPENDED, input_from_event};

use crate::service::{Service, lock};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł wyzwalaczy.
pub struct TriggersModule {
    manifest: ModuleManifest,
    scheduler: Arc<dyn Scheduler>,
    store: Arc<dyn TriggerStore>,
    files: Arc<dyn FileWatchPort>,
    start_ms: Option<u64>,
    running: Mutex<Option<Arc<Service>>>,
}

impl TriggersModule {
    /// Nowy moduł: scheduler (ujście zadań), magazyn stanu, obserwacja plików.
    pub fn new(
        scheduler: Arc<dyn Scheduler>,
        store: Arc<dyn TriggerStore>,
        files: Arc<dyn FileWatchPort>,
    ) -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            scheduler,
            store,
            files,
            start_ms: None,
            running: Mutex::new(None),
        })
    }

    /// Zegar od zadanej chwili (testy: przewidywalne daty, np. zmiana czasu).
    #[must_use]
    pub fn with_start_ms(mut self, start_ms: u64) -> Self {
        self.start_ms = Some(start_ms);
        self
    }

    fn svc(&self) -> Result<Arc<Service>, TriggerError> {
        lock(&self.running).clone().ok_or(TriggerError::NotStarted)
    }

    /// Obserwator plików zgłasza nowy plik (treść niezaufana — zadanie dostaje taint).
    pub fn file_created(&self, path: &str) -> Vec<RunRecord> {
        self.input(TriggerInput::FileCreated { path: path.into() })
    }

    /// Bieżący czas modułu (ms UTC).
    pub fn now_ms(&self) -> Option<u64> {
        use triggers_contract::TriggerHost;
        self.svc().ok().map(|s| s.core.host().now_ms())
    }
}

impl Triggers for TriggersModule {
    fn create(&self, spec: TriggerSpec, actor: Actor) -> Result<TriggerView, TriggerError> {
        self.svc()?.core.create(spec, actor)
    }

    fn update(&self, spec: TriggerSpec, actor: Actor) -> Result<TriggerView, TriggerError> {
        self.svc()?.core.update(spec, actor)
    }

    fn remove(&self, id: &TriggerId, actor: Actor) -> Result<(), TriggerError> {
        self.svc()?.core.remove(id, actor)
    }

    fn set_enabled(&self, id: &TriggerId, enabled: bool, actor: Actor) -> Result<(), TriggerError> {
        self.svc()?.core.set_enabled(id, enabled, actor)
    }

    fn fire_now(&self, id: &TriggerId, actor: Actor) -> Result<RunRecord, TriggerError> {
        self.svc()?.core.fire_now(id, actor)
    }

    fn input(&self, input: TriggerInput) -> Vec<RunRecord> {
        self.svc().map(|s| s.core.input(input)).unwrap_or_default()
    }

    fn set_dnd(&self, on: bool) {
        if let Ok(s) = self.svc() {
            s.core.set_dnd(on);
        }
    }

    fn list(&self) -> Vec<TriggerView> {
        self.svc().map(|s| s.core.list()).unwrap_or_default()
    }

    fn get(&self, id: &TriggerId) -> Option<TriggerView> {
        self.svc().ok()?.core.get(id)
    }

    fn log(&self, id: Option<&TriggerId>, limit: usize) -> Vec<RunRecord> {
        self.svc()
            .map(|s| s.core.log(id, limit))
            .unwrap_or_default()
    }
}

#[async_trait]
impl Module for TriggersModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        if lock(&self.running).is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        let svc = Service::start(
            ctx.bus,
            Arc::clone(&self.scheduler),
            Arc::clone(&self.store),
            Arc::clone(&self.files),
            self.start_ms,
        )
        .await
        .map_err(ModuleError::Other)?;
        *lock(&self.running) = Some(svc);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        let svc = lock(&self.running).take().ok_or(ModuleError::NotStarted)?;
        svc.shutdown().map_err(ModuleError::Other)
    }

    fn health(&self) -> HealthStatus {
        match self.running.try_lock() {
            Ok(guard) if guard.is_some() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::NotStarted,
            Err(_) => HealthStatus::Degraded("stan zajęty".into()),
        }
    }
}
