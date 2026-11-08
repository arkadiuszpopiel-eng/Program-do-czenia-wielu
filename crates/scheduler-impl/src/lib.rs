//! Implementacja pełnego `scheduler` (docs/modules/scheduler/SPEC.md, PLAN §9.3, §9.6).
//!
//! Rdzeń decyzyjny (`SchedCore`) pochodzi z kontraktu; ten crate dostarcza: zegar ścienny
//! (epoka + tokio `Instant`), sterownik terminów (timeouty, okna, ponowienia, przerwania siłą),
//! wykonawczynie jako przerywalne zadania tokio za portem [`TaskExecutor`] (adaptery w `app-*`:
//! `agent-runtime`, `agent-backends`, usługi), publikację `scheduler.*` na magistralę, zapis
//! stanu ([`FileSnapshotStore`], restart = wznowienie) i budżet tła z `cost-meter`
//! ([`CostMeterBudget`]). Jako nadzbiór `scheduler-lite` obsługuje dzierżawy mowy na tej samej
//! tablicy blokad — w kompozycji zastępuje `scheduler-lite-impl`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod ports;
mod service;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use scheduler_contract::{
    Holder, Lease, LeaseInfo, LeaseRequest, MemSnapshotStore, PreemptReason, QueuedRequest,
    Resource, Roster, SchedCore, SchedError, Scheduler, SchedulerLite, SnapshotStore, Steer,
    SystemConditions, TaskError, TaskExecutor, TaskId, TaskSpec, TaskView, Termination,
};

pub use ports::{BackgroundBudget, CostMeterBudget, FileSnapshotStore, UnlimitedBudget};
pub use service::{EVENT_PERSIST_FAILED, ImplHost};

use crate::service::{Service, lock};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Moduł pełnego schedulera.
pub struct SchedulerModule {
    manifest: ModuleManifest,
    executor: Arc<dyn TaskExecutor>,
    store: Arc<dyn SnapshotStore>,
    budget: Arc<dyn BackgroundBudget>,
    roster: Mutex<Option<Roster>>,
    conditions: Mutex<Option<SystemConditions>>,
    running: Mutex<Option<Arc<Service>>>,
}

impl SchedulerModule {
    /// Nowy moduł: wykonawczyni (port), magazyn stanu, budżet tła.
    pub fn new(
        executor: Arc<dyn TaskExecutor>,
        store: Arc<dyn SnapshotStore>,
        budget: Arc<dyn BackgroundBudget>,
    ) -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            executor,
            store,
            budget,
            roster: Mutex::new(None),
            conditions: Mutex::new(None),
            running: Mutex::new(None),
        })
    }

    /// Moduł ze stanem w pamięci i bez limitu tła (testy, prototypy).
    pub fn in_memory(executor: Arc<dyn TaskExecutor>) -> Result<Self, ManifestError> {
        Self::new(
            executor,
            Arc::new(MemSnapshotStore::new()),
            Arc::new(UnlimitedBudget),
        )
    }

    fn svc(&self) -> Result<Arc<Service>, TaskError> {
        lock(&self.running).clone().ok_or(TaskError::NotStarted)
    }

    /// Rdzeń decyzyjny (diagnostyka, panel Agentki: zasoby zadań).
    pub fn core(&self) -> Option<Arc<SchedCore<ImplHost>>> {
        lock(&self.running).as_ref().map(|s| Arc::clone(&s.core))
    }

    fn with_core<T>(
        &self,
        f: impl FnOnce(
            &SchedCore<ImplHost>,
        ) -> Result<(T, Vec<scheduler_contract::SchedEffect>), TaskError>,
    ) -> Result<T, TaskError> {
        let svc = self.svc()?;
        let (value, effects) = f(&svc.core)?;
        svc.apply(effects);
        Ok(value)
    }
}

#[async_trait]
impl SchedulerLite for SchedulerModule {
    async fn acquire(&self, request: LeaseRequest) -> Result<Lease, SchedError> {
        let svc = self.svc().map_err(|_| SchedError::NotStarted)?;
        let locks = Arc::clone(svc.core.locks());
        drop(svc);
        locks.acquire(request).await
    }

    fn preempt(
        &self,
        resource: &Resource,
        by: Holder,
        reason: PreemptReason,
    ) -> Result<(), SchedError> {
        let svc = self.svc().map_err(|_| SchedError::NotStarted)?;
        svc.core.locks().preempt(resource, by, reason)
    }

    fn handoff(&self, resource: &Resource, from: &Holder, to: Holder) -> Result<(), SchedError> {
        let svc = self.svc().map_err(|_| SchedError::NotStarted)?;
        svc.core.locks().handoff_from(resource, from, to)
    }

    fn holder(&self, resource: &Resource) -> Option<LeaseInfo> {
        self.svc().ok()?.core.locks().holder(resource)
    }

    fn queue(&self, resource: &Resource) -> Vec<QueuedRequest> {
        self.svc()
            .map(|s| s.core.locks().queue(resource))
            .unwrap_or_default()
    }

    /// Kill-switch: zadania (wykonawczynie przerwane od razu) i dzierżawy mowy.
    fn kill_all(&self) -> usize {
        self.with_core(|core| Ok(core.kill_all())).unwrap_or(0)
    }
}

#[async_trait]
impl Scheduler for SchedulerModule {
    fn submit(&self, tasks: Vec<TaskSpec>) -> Result<Vec<TaskId>, TaskError> {
        self.with_core(|core| core.submit(tasks))
    }

    fn cancel(&self, task: &TaskId, reason: &str) -> Result<Vec<TaskId>, TaskError> {
        self.with_core(|core| core.cancel(task, reason))
    }

    fn steer(&self, task: &TaskId, steer: Steer) -> Result<u64, TaskError> {
        self.with_core(|core| core.steer(task, steer))
    }

    fn pause(&self, task: &TaskId) -> Result<(), TaskError> {
        self.with_core(|core| core.pause(task).map(|fx| ((), fx)))
    }

    fn resume(&self, task: &TaskId) -> Result<(), TaskError> {
        self.with_core(|core| core.resume(task).map(|fx| ((), fx)))
    }

    fn task(&self, task: &TaskId) -> Option<TaskView> {
        self.svc().ok()?.core.task(task)
    }

    fn tasks(&self) -> Vec<TaskView> {
        self.svc().map(|s| s.core.tasks()).unwrap_or_default()
    }

    fn set_roster(&self, roster: Roster) {
        *lock(&self.roster) = Some(roster.clone());
        let _ = self.with_core(|core| Ok(((), core.set_roster(roster))));
    }

    fn set_conditions(&self, conditions: SystemConditions) {
        *lock(&self.conditions) = Some(conditions);
        let _ = self.with_core(|core| Ok(((), core.set_conditions(conditions))));
    }

    async fn wait(&self, task: &TaskId) -> Result<Termination, TaskError> {
        let svc = self.svc()?;
        loop {
            let notified = svc.finished.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let view = svc
                .core
                .task(task)
                .ok_or_else(|| TaskError::UnknownTask(task.clone()))?;
            if let Some(t) = view.state.termination() {
                return Ok(t.clone());
            }
            notified.await;
        }
    }
}

#[async_trait]
impl Module for SchedulerModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        if lock(&self.running).is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        let svc = Service::start(
            ctx.bus,
            Arc::clone(&self.executor),
            Arc::clone(&self.store),
            Arc::clone(&self.budget),
        )
        .map_err(ModuleError::Other)?;
        if let Some(roster) = lock(&self.roster).clone() {
            svc.apply(svc.core.set_roster(roster));
        }
        if let Some(conditions) = *lock(&self.conditions) {
            svc.apply(svc.core.set_conditions(conditions));
        }
        *lock(&self.running) = Some(svc);
        Ok(())
    }

    /// Zatrzymanie łagodne: stan zapisany, zadania w toku wznowią się po następnym starcie.
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
