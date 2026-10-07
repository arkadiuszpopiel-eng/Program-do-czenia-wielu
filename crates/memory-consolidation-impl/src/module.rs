//! Moduł zadania tła: co `interval` przebieg z harmonogramu (polityka sama odrzuca start poza
//! oknem, na baterii, w trybie gry, przy aktywnym użytkowniku), „Uporządkuj teraz” z UI.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use memory_consolidation_contract::{Guardian, RunReport, Trigger};
use tokio::task::JoinHandle;

/// Domyślny odstęp sprawdzania harmonogramu.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Moduł `memory-consolidation`.
pub struct ConsolidationModule {
    guardian: Arc<Guardian>,
    interval: Duration,
    task: Mutex<Option<JoinHandle<()>>>,
    last: Arc<Mutex<Option<RunReport>>>,
    manifest: ModuleManifest,
}

impl ConsolidationModule {
    /// Nowy moduł (harmonogram co `interval`).
    pub fn new(guardian: Guardian, interval: Duration) -> Result<Self, ManifestError> {
        Ok(Self {
            guardian: Arc::new(guardian),
            interval,
            task: Mutex::new(None),
            last: Arc::new(Mutex::new(None)),
            manifest: ModuleManifest::parse_toml(crate::MODULE_TOML)?,
        })
    }

    /// Przebieg na żądanie użytkownika (nadal nie na baterii ani w trybie gry).
    pub async fn run_now(&self) -> RunReport {
        let report = self.guardian.run(Trigger::Manual).await;
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = Some(report.clone());
        report
    }

    /// Ostatni raport (UI: „ostatnie porządkowanie pamięci”).
    pub fn last_report(&self) -> Option<RunReport> {
        self.last
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Strażniczka.
    pub fn guardian(&self) -> Arc<Guardian> {
        Arc::clone(&self.guardian)
    }
}

#[async_trait]
impl Module for ConsolidationModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, _ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut task = self.task.lock().unwrap_or_else(PoisonError::into_inner);
        if task.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        let guardian = Arc::clone(&self.guardian);
        let last = Arc::clone(&self.last);
        let interval = self.interval;
        *task = Some(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                let report = guardian.run(Trigger::Scheduled).await;
                if report.skipped.is_none() {
                    *last.lock().unwrap_or_else(PoisonError::into_inner) = Some(report);
                }
            }
        }));
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        match self
            .task
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            Some(handle) => {
                handle.abort();
                Ok(())
            }
            None => Err(ModuleError::NotStarted),
        }
    }

    fn health(&self) -> HealthStatus {
        if self
            .task
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
        {
            HealthStatus::Healthy
        } else {
            HealthStatus::NotStarted
        }
    }
}
