//! `Registry` dla atrapy: te same reguły grafu i cyklu życia co implementacja, czas wirtualny.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::{Event, Level};
use core_registry_contract::{
    ContractRef, DependencyGraph, EVENT_HEALTH, HealthStatus, Lifecycle, Module, ModuleContext,
    ModuleId, ModuleState, ModuleStatus, Registry, RegistryError, registry_event_kind,
};
use serde_json::json;

use crate::{FakeRegistry, Slot};

type Slots = BTreeMap<ModuleId, Slot>;

fn slot<'a>(slots: &'a Slots, id: &ModuleId) -> Result<&'a Slot, RegistryError> {
    slots
        .get(id)
        .ok_or_else(|| RegistryError::UnknownModule(id.clone()))
}

fn dependents(slots: &Slots, id: &ModuleId, running_only: bool) -> Vec<ModuleId> {
    let Some(target) = slots.get(id) else {
        return Vec::new();
    };
    slots
        .iter()
        .filter(|(other, s)| {
            *other != id
                && if running_only {
                    s.state.is_running()
                } else {
                    s.state != ModuleState::Disabled
                }
                && s.manifest
                    .requires
                    .iter()
                    .any(|r| target.manifest.provides.contains(r))
        })
        .map(|(other, _)| other.clone())
        .collect()
}

fn stop_order(
    slots: &Slots,
    id: &ModuleId,
    seen: &mut BTreeSet<ModuleId>,
    out: &mut Vec<ModuleId>,
) {
    if seen.insert(id.clone()) {
        for d in dependents(slots, id, true) {
            stop_order(slots, &d, seen, out);
        }
        out.push(id.clone());
    }
}

impl FakeRegistry {
    fn graph(&self, slots: &Slots) -> Result<DependencyGraph, RegistryError> {
        let enabled = slots
            .values()
            .filter(|s| s.state != ModuleState::Disabled)
            .map(|s| &s.manifest);
        DependencyGraph::build(enabled, &self.external)
    }

    async fn start(&self, slots: &mut Slots, id: &ModuleId) -> Result<bool, RegistryError> {
        let s = slot(slots, id)?;
        if s.state.is_running() {
            return Ok(false);
        }
        if s.state == ModuleState::Disabled {
            return Err(RegistryError::Disabled(id.clone()));
        }
        self.transition(slots, id, ModuleState::Loading).await;
        let ctx = ModuleContext::new(id.clone(), Arc::clone(&self.bus));
        let now = self.elapsed();
        let Some(s) = slots.get_mut(id) else {
            return Err(RegistryError::UnknownModule(id.clone()));
        };
        let result = if std::mem::take(&mut s.fail_next_start) {
            Err("wstrzyknięta awaria startu".to_owned())
        } else {
            s.module.start(ctx).await.map_err(|e| e.to_string())
        };
        match result {
            Ok(()) => {
                s.failures = 0;
                s.last_used = now;
                self.transition(slots, id, ModuleState::Ready).await;
                Ok(true)
            }
            Err(reason) => {
                s.failures = s.failures.saturating_add(1);
                let failed = ModuleState::Failed {
                    restarts: s.failures,
                    reason: reason.clone(),
                };
                self.transition(slots, id, failed).await;
                Err(RegistryError::StartFailed {
                    module: id.clone(),
                    reason,
                })
            }
        }
    }

    async fn stop(&self, slots: &mut Slots, id: &ModuleId) -> Result<bool, RegistryError> {
        let Some(s) = slots.get_mut(id) else {
            return Err(RegistryError::UnknownModule(id.clone()));
        };
        if !s.state.is_running() {
            return Ok(false);
        }
        if let Err(e) = s.module.stop().await {
            let failed = ModuleState::Failed {
                restarts: s.failures,
                reason: e.to_string(),
            };
            self.transition(slots, id, failed).await;
            return Err(RegistryError::StopFailed {
                module: id.clone(),
                reason: e.to_string(),
            });
        }
        self.transition(slots, id, ModuleState::Unloaded).await;
        Ok(true)
    }

    async fn ensure(
        &self,
        slots: &mut Slots,
        graph: &DependencyGraph,
        id: &ModuleId,
    ) -> Result<Vec<ModuleId>, RegistryError> {
        let mut started = Vec::new();
        for m in graph.start_closure(id) {
            if self.start(slots, &m).await? {
                started.push(m);
            }
        }
        Ok(started)
    }

    fn touch(&self, slots: &mut Slots, id: &ModuleId) {
        let now = self.elapsed();
        if let Some(s) = slots.get_mut(id) {
            s.last_used = now;
        }
    }
}

#[async_trait]
impl Registry for FakeRegistry {
    async fn register(&self, module: Box<dyn Module>) -> Result<(), RegistryError> {
        let manifest = module.manifest().clone();
        self.note(format!("register:{}", manifest.id));
        let mut slots = self.slots.lock().await;
        if slots.contains_key(&manifest.id) {
            return Err(RegistryError::Duplicate(manifest.id));
        }
        let last_used = self.elapsed();
        let id = manifest.id.clone();
        let s = Slot {
            module,
            manifest,
            state: ModuleState::Unloaded,
            last_used,
            failures: 0,
            fail_next_start: false,
        };
        slots.insert(id, s);
        Ok(())
    }

    async fn start_order(&self) -> Result<Vec<ModuleId>, RegistryError> {
        let slots = self.slots.lock().await;
        Ok(self.graph(&slots)?.order().to_vec())
    }

    async fn boot(&self) -> Result<Vec<ModuleId>, RegistryError> {
        self.note("boot".into());
        let mut slots = self.slots.lock().await;
        let graph = self.graph(&slots)?;
        let mut started = Vec::new();
        for id in graph.order() {
            if slot(&slots, id)?.manifest.lifecycle == Lifecycle::Always {
                started.extend(self.ensure(&mut slots, &graph, id).await?);
            }
        }
        Ok(started)
    }

    async fn activate(&self, id: &ModuleId) -> Result<(), RegistryError> {
        self.note(format!("activate:{id}"));
        let mut slots = self.slots.lock().await;
        if slot(&slots, id)?.state == ModuleState::Disabled {
            return Err(RegistryError::Disabled(id.clone()));
        }
        let graph = self.graph(&slots)?;
        self.ensure(&mut slots, &graph, id).await?;
        self.touch(&mut slots, id);
        Ok(())
    }

    async fn acquire(&self, contract: &ContractRef) -> Result<ModuleId, RegistryError> {
        self.note(format!("acquire:{contract}"));
        let mut slots = self.slots.lock().await;
        let graph = self.graph(&slots)?;
        let provider = graph
            .provider(contract)
            .cloned()
            .ok_or_else(|| RegistryError::NoProvider(contract.clone()))?;
        let s = slot(&slots, &provider)?;
        if !s.state.is_running() && s.manifest.lifecycle == Lifecycle::OnDemand {
            return Err(RegistryError::NotActivated(provider));
        }
        self.ensure(&mut slots, &graph, &provider).await?;
        self.touch(&mut slots, &provider);
        Ok(provider)
    }

    async fn deactivate(&self, id: &ModuleId) -> Result<Vec<ModuleId>, RegistryError> {
        self.note(format!("deactivate:{id}"));
        let mut slots = self.slots.lock().await;
        if slot(&slots, id)?.manifest.lifecycle == Lifecycle::Always {
            return Err(RegistryError::Resident(id.clone()));
        }
        let mut order = Vec::new();
        stop_order(&slots, id, &mut BTreeSet::new(), &mut order);
        let resident: Vec<ModuleId> = order
            .iter()
            .filter(|m| {
                *m != id
                    && slots
                        .get(*m)
                        .is_some_and(|s| s.manifest.lifecycle == Lifecycle::Always)
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
            if self.stop(&mut slots, &m).await? {
                stopped.push(m);
            }
        }
        Ok(stopped)
    }

    async fn set_enabled(&self, id: &ModuleId, enabled: bool) -> Result<(), RegistryError> {
        self.note(format!("set_enabled:{id}:{enabled}"));
        let mut slots = self.slots.lock().await;
        let disabled = slot(&slots, id)?.state == ModuleState::Disabled;
        if enabled || disabled {
            if enabled && disabled {
                self.transition(&mut slots, id, ModuleState::Unloaded).await;
            }
            return Ok(());
        }
        let users = dependents(&slots, id, false);
        if !users.is_empty() {
            return Err(RegistryError::InUse {
                module: id.clone(),
                dependents: users,
            });
        }
        self.stop(&mut slots, id).await?;
        self.transition(&mut slots, id, ModuleState::Disabled).await;
        Ok(())
    }

    async fn list(&self) -> Vec<ModuleStatus> {
        let slots = self.slots.lock().await;
        slots
            .iter()
            .map(|(id, s)| ModuleStatus {
                id: id.clone(),
                version: s.manifest.version.clone(),
                lifecycle: s.manifest.lifecycle,
                state: s.state.clone(),
                provides: s.manifest.provides.clone(),
            })
            .collect()
    }

    async fn health(&self, id: &ModuleId) -> Result<HealthStatus, RegistryError> {
        let slots = self.slots.lock().await;
        let s = slot(&slots, id)?;
        let status = match &s.state {
            ModuleState::Degraded { reason } => HealthStatus::Degraded(reason.clone()),
            ModuleState::Failed { reason, .. } => HealthStatus::Unhealthy(reason.clone()),
            _ => s.module.health(),
        };
        let label = match &status {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded(_) => "degraded",
            HealthStatus::Unhealthy(_) => "unhealthy",
            HealthStatus::NotStarted => "not-started",
        };
        let payload = json!({"module": id.as_str(), "status": label});
        let event = Event::new(registry_event_kind(EVENT_HEALTH), Level::Debug, payload);
        let _ = self.bus.publish(event).await;
        Ok(status)
    }

    async fn unload_idle(&self) -> Result<Vec<ModuleId>, RegistryError> {
        let now = self.elapsed();
        let mut slots = self.slots.lock().await;
        let mut unloaded = Vec::new();
        loop {
            let next = slots
                .iter()
                .filter(|(_, s)| {
                    s.state.is_running()
                        && s.manifest.lifecycle != Lifecycle::Always
                        && now.saturating_sub(s.last_used) > self.idle_timeout
                })
                .map(|(id, _)| id.clone())
                .find(|id| dependents(&slots, id, true).is_empty());
            let Some(id) = next else {
                return Ok(unloaded);
            };
            self.stop(&mut slots, &id).await?;
            unloaded.push(id);
        }
    }

    async fn shutdown(&self) -> Result<Vec<ModuleId>, RegistryError> {
        self.note("shutdown".into());
        let mut slots = self.slots.lock().await;
        let mut stopped = Vec::new();
        loop {
            let next = slots
                .iter()
                .filter(|(_, s)| s.state.is_running())
                .map(|(id, _)| id.clone())
                .find(|id| dependents(&slots, id, true).is_empty());
            let Some(id) = next else {
                return Ok(stopped);
            };
            self.stop(&mut slots, &id).await?;
            stopped.push(id);
        }
    }
}
