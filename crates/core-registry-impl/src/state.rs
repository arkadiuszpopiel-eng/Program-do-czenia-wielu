//! Stan wewnętrzny rejestru i przejścia cyklu życia (start/stop pojedynczego modułu).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{
    DependencyGraph, EVENT_STATE_CHANGED, Module, ModuleContext, ModuleId, ModuleManifest,
    ModuleState, RegistryError, registry_event_kind,
};
use serde_json::json;

use crate::config::{Clock, RegistryConfig};

/// Zależności środowiska potrzebne przejściom stanu.
pub(crate) struct Env<'a> {
    pub bus: &'a Arc<dyn EventBus>,
    pub clock: &'a dyn Clock,
    pub config: &'a RegistryConfig,
}

impl Env<'_> {
    /// Publikuje zdarzenie rejestru; błąd magistrali nie przerywa operacji (zdarzenie diagnostyczne).
    pub async fn publish(&self, name: &str, level: Level, payload: serde_json::Value) {
        let event = Event::new(registry_event_kind(name), level, payload);
        if let Err(e) = self.bus.publish(event).await {
            tracing::warn!(error = %e, zdarzenie = name, "nie opublikowano zdarzenia rejestru");
        }
    }
}

/// Wpis modułu.
pub(crate) struct Entry {
    pub module: Box<dyn Module>,
    pub manifest: ModuleManifest,
    pub state: ModuleState,
    pub last_used: DateTime<Utc>,
    pub failures: u8,
}

/// Wszystkie wpisy (posortowane po `id`).
#[derive(Default)]
pub(crate) struct Inner {
    pub entries: BTreeMap<ModuleId, Entry>,
}

impl Inner {
    pub fn entry(&self, id: &ModuleId) -> Result<&Entry, RegistryError> {
        self.entries
            .get(id)
            .ok_or_else(|| RegistryError::UnknownModule(id.clone()))
    }

    pub fn entry_mut(&mut self, id: &ModuleId) -> Result<&mut Entry, RegistryError> {
        self.entries
            .get_mut(id)
            .ok_or_else(|| RegistryError::UnknownModule(id.clone()))
    }

    /// Graf włączonych modułów (walidacja braków, konfliktów i cykli).
    pub fn graph(&self, env: &Env<'_>) -> Result<DependencyGraph, RegistryError> {
        let enabled = self
            .entries
            .values()
            .filter(|e| e.state != ModuleState::Disabled)
            .map(|e| &e.manifest);
        DependencyGraph::build(enabled, &env.config.external_contracts)
    }

    /// Zmienia stan i publikuje `registry.module.state_changed` (tylko przy faktycznej zmianie).
    pub async fn transition(&mut self, env: &Env<'_>, id: &ModuleId, to: ModuleState) {
        let Some(entry) = self.entries.get_mut(id) else {
            return;
        };
        if entry.state == to {
            return;
        }
        let from = std::mem::replace(&mut entry.state, to.clone());
        let (level, reason) = match &to {
            ModuleState::Failed { reason, .. } => (Level::Warn, Some(reason.clone())),
            ModuleState::Degraded { reason } => (Level::Warn, Some(reason.clone())),
            _ => (Level::Info, None),
        };
        let mut payload = json!({"module": id.as_str(), "from": from.name(), "to": to.name()});
        if let (Some(reason), Some(obj)) = (reason, payload.as_object_mut()) {
            obj.insert("reason".into(), reason.into());
        }
        env.publish(EVENT_STATE_CHANGED, level, payload).await;
    }

    /// Odnotowuje użycie modułu (licznik bezczynności).
    pub fn touch(&mut self, id: &ModuleId, now: DateTime<Utc>) {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.last_used = now;
        }
    }

    /// Uruchamia jeden moduł (bez zależności). `Ok(true)` = wystartował teraz.
    pub async fn start_one(&mut self, env: &Env<'_>, id: &ModuleId) -> Result<bool, RegistryError> {
        let limit = env.config.crash_loop_limit;
        let entry = self.entry(id)?;
        if entry.state.is_running() {
            return Ok(false);
        }
        if entry.state == ModuleState::Disabled {
            return Err(RegistryError::Disabled(id.clone()));
        }
        if entry.failures >= limit {
            return Err(RegistryError::CrashLoop {
                module: id.clone(),
                restarts: entry.failures,
            });
        }
        self.transition(env, id, ModuleState::Loading).await;
        let ctx = ModuleContext::new(id.clone(), Arc::clone(env.bus));
        let entry = self.entry_mut(id)?;
        let result = entry.module.start(ctx).await;
        match result {
            Ok(()) => {
                entry.failures = 0;
                entry.last_used = env.clock.now();
                self.transition(env, id, ModuleState::Ready).await;
                Ok(true)
            }
            Err(e) => {
                entry.failures = entry.failures.saturating_add(1);
                let failed = ModuleState::Failed {
                    restarts: entry.failures,
                    reason: e.to_string(),
                };
                self.transition(env, id, failed).await;
                Err(RegistryError::StartFailed {
                    module: id.clone(),
                    reason: e.to_string(),
                })
            }
        }
    }

    /// Zatrzymuje jeden moduł (bez zależnych). `Ok(true)` = zatrzymany teraz.
    pub async fn stop_one(&mut self, env: &Env<'_>, id: &ModuleId) -> Result<bool, RegistryError> {
        let entry = self.entry_mut(id)?;
        if !entry.state.is_running() {
            return Ok(false);
        }
        let result = entry.module.stop().await;
        match result {
            Ok(()) => {
                self.transition(env, id, ModuleState::Unloaded).await;
                Ok(true)
            }
            Err(e) => {
                let failed = ModuleState::Failed {
                    restarts: entry.failures,
                    reason: format!("zatrzymanie: {e}"),
                };
                self.transition(env, id, failed).await;
                Err(RegistryError::StopFailed {
                    module: id.clone(),
                    reason: e.to_string(),
                })
            }
        }
    }

    /// Uruchamia moduł wraz z zależnościami w kolejności grafu; zwraca uruchomione teraz.
    pub async fn ensure_running(
        &mut self,
        env: &Env<'_>,
        graph: &DependencyGraph,
        id: &ModuleId,
    ) -> Result<Vec<ModuleId>, RegistryError> {
        let mut started = Vec::new();
        for m in graph.start_closure(id) {
            if self.start_one(env, &m).await? {
                started.push(m);
            }
        }
        Ok(started)
    }

    /// Uruchomione moduły wymagające kontraktu dostarczanego przez `id` (po `id`).
    pub fn running_dependents(&self, id: &ModuleId) -> Vec<ModuleId> {
        let Some(target) = self.entries.get(id) else {
            return Vec::new();
        };
        self.entries
            .iter()
            .filter(|(other, e)| {
                *other != id
                    && e.state.is_running()
                    && e.manifest
                        .requires
                        .iter()
                        .any(|r| target.manifest.provides.contains(r))
            })
            .map(|(other, _)| other.clone())
            .collect()
    }

    /// Kolejność zatrzymania: najpierw uruchomione moduły zależne (przechodnio), `id` na końcu.
    /// Liczona z manifestów uruchomionych modułów — działa także przy grafie niepoprawnym.
    pub fn stop_order(&self, id: &ModuleId) -> Vec<ModuleId> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        self.visit_dependents(id, &mut seen, &mut out);
        out
    }

    fn visit_dependents(
        &self,
        id: &ModuleId,
        seen: &mut BTreeSet<ModuleId>,
        out: &mut Vec<ModuleId>,
    ) {
        if !seen.insert(id.clone()) {
            return;
        }
        for dependent in self.running_dependents(id) {
            self.visit_dependents(&dependent, seen, out);
        }
        out.push(id.clone());
    }
}
