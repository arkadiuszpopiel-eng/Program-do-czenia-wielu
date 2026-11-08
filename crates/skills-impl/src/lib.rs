//! Implementacja modułu `skills` (docs/modules/skills/SPEC.md, PLAN §9.5, §12.1 R1).
//!
//! Rdzeń decyzyjny ([`SkillLibrary`]: walidacja, testy akceptacyjne, skaner, kwarantanna,
//! zatwierdzenia z hashem, wersje) pochodzi z kontraktu; ten crate dodaje trwały magazyn
//! ([`DirSkillStore`], zapis atomowy, wycofanie zmiany przy błędzie zapisu), cykl życia
//! modułu, publikację `skills.*` na magistrali, uruchamianie przez `agent-runtime`
//! ([`SkillRunner`]) i dokument `.alfa` kategorii `skills` ([`SkillsDocuments`]).
//! Zmiany wymagają uruchomionego modułu — każda trafia do dziennika zdarzeń.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod documents;
mod runner;
mod store;

use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use personas_contract::Role;
use semver::Version;
use skills_contract::{
    ImportOrigin, ImportReport, OwnerApproval, Skill, SkillBundle, SkillError, SkillId,
    SkillLibrary, SkillMatch, SkillRecord, SkillSource, Skills, search,
};
use tools_common_contract::ToolManifest;

pub use documents::SkillsDocuments;
pub use runner::SkillRunner;
pub use store::{DirSkillStore, MemSkillStore, SkillStore};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu.
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

fn system_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Wynik zmiany: wartość, zdarzenia do publikacji, magistrala.
pub(crate) type Changed<T> = (T, Vec<Event>, Arc<dyn EventBus>);

/// Moduł biblioteki umiejętności.
pub struct SkillsModule {
    manifest: ModuleManifest,
    lib: Mutex<SkillLibrary>,
    store: Arc<dyn SkillStore>,
    bus: RwLock<Option<Arc<dyn EventBus>>>,
    clock: fn() -> u64,
}

impl SkillsModule {
    /// Moduł nad katalogiem narzędzi i magazynem (wersje wczytane z magazynu).
    pub fn new(catalog: Vec<ToolManifest>, store: Arc<dyn SkillStore>) -> Result<Self, String> {
        Self::with_clock(catalog, store, system_now_ms)
    }

    /// Jak [`SkillsModule::new`], z zegarem (ms; testy).
    pub fn with_clock(
        catalog: Vec<ToolManifest>,
        store: Arc<dyn SkillStore>,
        clock: fn() -> u64,
    ) -> Result<Self, String> {
        let records = store.load()?;
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML).map_err(|e| e.to_string())?,
            lib: Mutex::new(SkillLibrary::new(catalog, records)),
            store,
            bus: RwLock::new(None),
            clock,
        })
    }

    fn lib(&self) -> MutexGuard<'_, SkillLibrary> {
        self.lib.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn bus(&self) -> Result<Arc<dyn EventBus>, SkillError> {
        self.bus
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| SkillError::Store("moduł umiejętności nie jest uruchomiony".into()))
    }

    /// Zmiana stanu: na kopii, zapis do magazynu, dopiero potem podmiana (błąd zapisu = brak zmiany).
    pub(crate) fn mutate<T>(
        &self,
        f: impl FnOnce(&mut SkillLibrary, u64) -> Result<(T, Vec<Event>), SkillError>,
    ) -> Result<Changed<T>, SkillError> {
        let bus = self.bus()?;
        let now = (self.clock)();
        let mut lib = self.lib();
        let mut next = lib.clone();
        let (value, events) = f(&mut next, now)?;
        if !events.is_empty() {
            self.store.save(next.records()).map_err(SkillError::Store)?;
        }
        *lib = next;
        Ok((value, events, bus))
    }

    async fn change<T>(
        &self,
        f: impl FnOnce(&mut SkillLibrary, u64) -> Result<(T, Vec<Event>), SkillError>,
    ) -> Result<T, SkillError> {
        let (value, events, bus) = self.mutate(f)?;
        for e in events {
            let _ = bus.publish(e).await;
        }
        Ok(value)
    }
}

#[async_trait]
impl Skills for SkillsModule {
    fn catalog(&self) -> Vec<ToolManifest> {
        self.lib().catalog().to_vec()
    }

    fn list(&self) -> Vec<SkillRecord> {
        self.lib().records().to_vec()
    }

    fn installed(&self, id: &SkillId) -> Option<SkillRecord> {
        self.lib().installed(id).cloned()
    }

    async fn propose(&self, skill: Skill, source: SkillSource) -> Result<SkillRecord, SkillError> {
        self.change(|l, now| l.propose(skill, source, now)).await
    }

    async fn approve(
        &self,
        id: &SkillId,
        version: &Version,
        approval: OwnerApproval,
    ) -> Result<SkillRecord, SkillError> {
        self.change(|l, now| l.approve(id, version, approval, now))
            .await
    }

    async fn release(
        &self,
        id: &SkillId,
        version: &Version,
        approval: OwnerApproval,
    ) -> Result<SkillRecord, SkillError> {
        self.change(|l, now| l.release(id, version, approval, now))
            .await
    }

    async fn reject(&self, id: &SkillId, version: &Version) -> Result<SkillRecord, SkillError> {
        self.change(|l, now| l.reject(id, version, now)).await
    }

    async fn disable(&self, id: &SkillId) -> Result<SkillRecord, SkillError> {
        self.change(|l, now| l.disable(id, now)).await
    }

    fn search(&self, task: &str, caller_roles: &[Role], limit: usize) -> Vec<SkillMatch> {
        let l = self.lib();
        search(l.records(), l.catalog(), caller_roles, task, limit)
    }

    fn export(&self, ids: &[SkillId]) -> Result<SkillBundle, SkillError> {
        self.lib().export(ids)
    }

    async fn import(
        &self,
        bundle: &SkillBundle,
        origin: ImportOrigin,
    ) -> Result<ImportReport, SkillError> {
        self.change(|l, now| l.import(bundle, origin, now)).await
    }
}

#[async_trait]
impl Module for SkillsModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut bus = self.bus.write().unwrap_or_else(|p| p.into_inner());
        if bus.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        *bus = Some(ctx.bus);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        let mut bus = self.bus.write().unwrap_or_else(|p| p.into_inner());
        bus.take().map(|_| ()).ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        match self.bus.try_read() {
            Ok(b) if b.is_some() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::NotStarted,
            Err(_) => HealthStatus::Degraded("stan zajęty".into()),
        }
    }
}
