//! `ModuleRegistry` — produkcyjna implementacja traitu `Registry`.

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::{EventBus, Level};
use core_registry_contract::{
    ContractRef, EVENT_HEALTH, EVENT_RESOLVE_FAILED, HealthStatus, Lifecycle, Module, ModuleId,
    ModuleState, ModuleStatus, Registry, RegistryError,
};
use serde_json::json;
use tokio::sync::Mutex;

use crate::config::{Clock, RegistryConfig, SystemClock};
use crate::state::{Entry, Env, Inner};

/// Rejestr modułów in-proc. Operacje są serializowane (jeden zamek async), więc `Module::start`
/// nie może wołać rejestru (zakleszczenie) — moduły dostają zależności przez `ModuleContext`.
pub struct ModuleRegistry {
    inner: Mutex<Inner>,
    bus: Arc<dyn EventBus>,
    clock: Arc<dyn Clock>,
    config: RegistryConfig,
}

impl ModuleRegistry {
    /// Rejestr z zegarem systemowym i konfiguracją domyślną.
    pub fn new(bus: Arc<dyn EventBus>) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            bus,
            clock: Arc::new(SystemClock),
            config: RegistryConfig::default(),
        }
    }

    /// Ustawia konfigurację (builder).
    #[must_use]
    pub fn with_config(mut self, config: RegistryConfig) -> Self {
        self.config = config;
        self
    }

    /// Ustawia zegar (builder) — w testach wirtualny.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Bieżąca konfiguracja.
    pub fn config(&self) -> &RegistryConfig {
        &self.config
    }

    fn env(&self) -> Env<'_> {
        Env {
            bus: &self.bus,
            clock: self.clock.as_ref(),
            config: &self.config,
        }
    }

    async fn resolve_failed(&self, contract: &ContractRef, err: RegistryError) -> RegistryError {
        let payload = json!({"contract": contract.to_string(), "reason": err.to_string()});
        self.env()
            .publish(EVENT_RESOLVE_FAILED, Level::Warn, payload)
            .await;
        err
    }
}

fn health_label(status: &HealthStatus) -> (&'static str, Option<&str>) {
    match status {
        HealthStatus::Healthy => ("healthy", None),
        HealthStatus::Degraded(d) => ("degraded", Some(d)),
        HealthStatus::Unhealthy(d) => ("unhealthy", Some(d)),
        HealthStatus::NotStarted => ("not-started", None),
    }
}

#[async_trait]
impl Registry for ModuleRegistry {
    async fn register(&self, module: Box<dyn Module>) -> Result<(), RegistryError> {
        let manifest = module.manifest().clone();
        let mut inner = self.inner.lock().await;
        if inner.entries.contains_key(&manifest.id) {
            return Err(RegistryError::Duplicate(manifest.id));
        }
        let entry = Entry {
            module,
            manifest: manifest.clone(),
            state: ModuleState::Unloaded,
            last_used: self.clock.now(),
            failures: 0,
        };
        inner.entries.insert(manifest.id, entry);
        Ok(())
    }

    async fn start_order(&self) -> Result<Vec<ModuleId>, RegistryError> {
        let inner = self.inner.lock().await;
        Ok(inner.graph(&self.env())?.order().to_vec())
    }

    async fn boot(&self) -> Result<Vec<ModuleId>, RegistryError> {
        let env = self.env();
        let mut inner = self.inner.lock().await;
        let graph = inner.graph(&env)?;
        let mut started = Vec::new();
        for id in graph.order() {
            if inner.entry(id)?.manifest.lifecycle == Lifecycle::Always {
                started.extend(inner.ensure_running(&env, &graph, id).await?);
            }
        }
        Ok(started)
    }

    async fn activate(&self, id: &ModuleId) -> Result<(), RegistryError> {
        let env = self.env();
        let mut inner = self.inner.lock().await;
        if inner.entry(id)?.state == ModuleState::Disabled {
            return Err(RegistryError::Disabled(id.clone()));
        }
        let graph = inner.graph(&env)?;
        inner.ensure_running(&env, &graph, id).await?;
        inner.touch(id, self.clock.now());
        Ok(())
    }

    async fn acquire(&self, contract: &ContractRef) -> Result<ModuleId, RegistryError> {
        let env = self.env();
        let mut inner = self.inner.lock().await;
        let graph = match inner.graph(&env) {
            Ok(graph) => graph,
            Err(e) => return Err(self.resolve_failed(contract, e).await),
        };
        let Some(provider) = graph.provider(contract).cloned() else {
            let err = RegistryError::NoProvider(contract.clone());
            return Err(self.resolve_failed(contract, err).await);
        };
        let entry = inner.entry(&provider)?;
        if !entry.state.is_running() && entry.manifest.lifecycle == Lifecycle::OnDemand {
            let err = RegistryError::NotActivated(provider);
            return Err(self.resolve_failed(contract, err).await);
        }
        if let Err(e) = inner.ensure_running(&env, &graph, &provider).await {
            return Err(self.resolve_failed(contract, e).await);
        }
        inner.touch(&provider, self.clock.now());
        Ok(provider)
    }

    async fn deactivate(&self, id: &ModuleId) -> Result<Vec<ModuleId>, RegistryError> {
        let env = self.env();
        let mut inner = self.inner.lock().await;
        if inner.entry(id)?.manifest.lifecycle == Lifecycle::Always {
            return Err(RegistryError::Resident(id.clone()));
        }
        let order = inner.stop_order(id);
        let resident: Vec<ModuleId> = order
            .iter()
            .filter(|m| {
                *m != id
                    && inner
                        .entries
                        .get(*m)
                        .is_some_and(|e| e.manifest.lifecycle == Lifecycle::Always)
            })
            .cloned()
            .collect();
        if !resident.is_empty() {
            return Err(RegistryError::InUse {
                module: id.clone(),
                dependents: resident,
            });
        }
        let mut stopped = Vec::new();
        for m in order {
            if inner.stop_one(&env, &m).await? {
                stopped.push(m);
            }
        }
        Ok(stopped)
    }

    async fn set_enabled(&self, id: &ModuleId, enabled: bool) -> Result<(), RegistryError> {
        let env = self.env();
        let mut inner = self.inner.lock().await;
        let entry = inner.entry(id)?;
        let is_disabled = entry.state == ModuleState::Disabled;
        if enabled {
            if is_disabled {
                inner.entry_mut(id)?.failures = 0;
                inner.transition(&env, id, ModuleState::Unloaded).await;
            }
            return Ok(());
        }
        if is_disabled {
            return Ok(());
        }
        let provides = entry.manifest.provides.clone();
        let dependents: Vec<ModuleId> = inner
            .entries
            .iter()
            .filter(|(other, e)| {
                *other != id
                    && e.state != ModuleState::Disabled
                    && e.manifest.requires.iter().any(|r| provides.contains(r))
            })
            .map(|(other, _)| other.clone())
            .collect();
        if !dependents.is_empty() {
            return Err(RegistryError::InUse {
                module: id.clone(),
                dependents,
            });
        }
        inner.stop_one(&env, id).await?;
        inner.transition(&env, id, ModuleState::Disabled).await;
        Ok(())
    }

    async fn list(&self) -> Vec<ModuleStatus> {
        let inner = self.inner.lock().await;
        inner
            .entries
            .iter()
            .map(|(id, e)| ModuleStatus {
                id: id.clone(),
                version: e.manifest.version.clone(),
                lifecycle: e.manifest.lifecycle,
                state: e.state.clone(),
                provides: e.manifest.provides.clone(),
            })
            .collect()
    }

    async fn health(&self, id: &ModuleId) -> Result<HealthStatus, RegistryError> {
        let env = self.env();
        let mut inner = self.inner.lock().await;
        let entry = inner.entry(id)?;
        let status = entry.module.health();
        if entry.state.is_running() {
            let next = match &status {
                HealthStatus::Healthy | HealthStatus::NotStarted => ModuleState::Ready,
                HealthStatus::Degraded(d) => ModuleState::Degraded { reason: d.clone() },
                HealthStatus::Unhealthy(d) => ModuleState::Degraded {
                    reason: format!("niezdrowy: {d}"),
                },
            };
            inner.transition(&env, id, next).await;
        }
        let (label, detail) = health_label(&status);
        let level = if label == "healthy" {
            Level::Debug
        } else {
            Level::Warn
        };
        let payload = json!({"module": id.as_str(), "status": label, "detail": detail});
        env.publish(EVENT_HEALTH, level, payload).await;
        Ok(status)
    }

    async fn unload_idle(&self) -> Result<Vec<ModuleId>, RegistryError> {
        let env = self.env();
        let now = self.clock.now();
        let mut inner = self.inner.lock().await;
        let mut unloaded = Vec::new();
        loop {
            // Jeden na raz: po zatrzymaniu zależnego jego zależności mogą stać się kandydatami.
            let next = inner
                .entries
                .iter()
                .filter(|(id, e)| {
                    let idle = (now - e.last_used).to_std().unwrap_or_default();
                    e.state.is_running()
                        && e.manifest.lifecycle != Lifecycle::Always
                        && idle > self.config.idle_limit(id)
                })
                .map(|(id, _)| id.clone())
                .find(|id| inner.running_dependents(id).is_empty());
            let Some(id) = next else {
                break;
            };
            inner.stop_one(&env, &id).await?;
            unloaded.push(id);
        }
        Ok(unloaded)
    }

    async fn shutdown(&self) -> Result<Vec<ModuleId>, RegistryError> {
        let env = self.env();
        let mut inner = self.inner.lock().await;
        let mut stopped = Vec::new();
        let mut first_error = None;
        loop {
            // Nieudane zatrzymanie zostawia `Failed` (nieuruchomiony), więc pętla się kończy.
            let next = inner
                .entries
                .iter()
                .filter(|(_, e)| e.state.is_running())
                .map(|(id, _)| id.clone())
                .find(|id| inner.running_dependents(id).is_empty());
            let Some(id) = next else {
                break;
            };
            match inner.stop_one(&env, &id).await {
                Ok(_) => stopped.push(id),
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        match first_error {
            Some(e) => Err(e),
            None => Ok(stopped),
        }
    }
}
