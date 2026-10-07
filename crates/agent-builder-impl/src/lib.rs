//! Implementacja modułu `agent-builder` — Kreator agentów (docs/modules/agent-builder/SPEC.md).
//!
//! Rdzeń ([`BuilderCore`]: budowa, polityka, test na sucho, zapis dwufazowy) pochodzi
//! z kontraktu; ten crate dodaje: katalog person i ról odświeżany z usługi `personas` (kolizje
//! z personami z innych źródeł), sufit autonomii z bieżącej sesji ([`CeilingSource`] — np.
//! poziom z Brokera; nigdy powyżej L3), zapis zatwierdzonej agentki do `personas` (rola, potem
//! persona) i do biblioteki manifestów ([`DirManifestStore`]), zdarzenia `agent_builder.*`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod store;

use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use agent_builder_contract::{
    AgentBuilder, AgentDraft, AgentManifest, BuildError, BuilderApproval, BuilderCore,
    BuilderPolicy, Built, DryRunReport, DryScenario, Preview, SavedAgent,
};
use async_trait::async_trait;
use core_bus_contract::{Event, EventBus};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use personas_contract::{Catalog, Personas};
use risk_classifier_contract::AutonomyLevel;
use tools_common_contract::ToolManifest;

pub use store::{DirManifestStore, ManifestStore, MemManifestStore};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu.
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Źródło sufitu autonomii nowej agentki (poziom sesji, w której działa Kreator).
pub trait CeilingSource: Send + Sync {
    /// Bieżący poziom (Kreator i tak przycina do L3).
    fn ceiling(&self) -> AutonomyLevel;
}

/// Stały sufit (testy, brak Brokera — wtedy bezpieczniej L2).
pub struct FixedCeiling(pub AutonomyLevel);

impl CeilingSource for FixedCeiling {
    fn ceiling(&self) -> AutonomyLevel {
        self.0
    }
}

/// Katalog: wbudowane + elementy własne z usługi `personas`.
fn catalog_of(personas: &dyn Personas) -> Catalog {
    let mut c = Catalog::builtin();
    for r in personas.roles().into_iter().filter(|r| !r.builtin) {
        let _ = c.add_role(r);
    }
    for p in personas.personas().into_iter().filter(|p| !p.builtin) {
        let _ = c.add_persona(p);
    }
    c
}

/// Moduł Kreatora agentów.
pub struct AgentBuilderModule {
    manifest: ModuleManifest,
    core: Mutex<BuilderCore>,
    personas: Arc<dyn Personas>,
    store: Arc<dyn ManifestStore>,
    ceiling: Arc<dyn CeilingSource>,
    bus: RwLock<Option<Arc<dyn EventBus>>>,
}

impl AgentBuilderModule {
    /// Moduł nad usługą person, katalogiem narzędzi, magazynem manifestów i źródłem sufitu.
    pub fn new(
        personas: Arc<dyn Personas>,
        tools: Vec<ToolManifest>,
        store: Arc<dyn ManifestStore>,
        ceiling: Arc<dyn CeilingSource>,
    ) -> Result<Self, String> {
        let mut core = BuilderCore::new(
            BuilderPolicy::with_ceiling(ceiling.ceiling()),
            tools,
            catalog_of(personas.as_ref()),
            voice_tts_contract::v0_chains(),
        );
        core.restore(store.load()?);
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML).map_err(|e| e.to_string())?,
            core: Mutex::new(core),
            personas,
            store,
            ceiling,
            bus: RwLock::new(None),
        })
    }

    /// Rdzeń z odświeżonym katalogiem i sufitem.
    fn core(&self) -> MutexGuard<'_, BuilderCore> {
        let mut c = self.core.lock().unwrap_or_else(|p| p.into_inner());
        c.set_catalog(catalog_of(self.personas.as_ref()));
        c.set_ceiling(self.ceiling.ceiling());
        c
    }

    fn bus(&self) -> Result<Arc<dyn EventBus>, BuildError> {
        self.bus
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| BuildError::Store("Kreator nie jest uruchomiony".into()))
    }

    async fn publish(bus: &Arc<dyn EventBus>, events: Vec<Event>) {
        for e in events {
            let _ = bus.publish(e).await;
        }
    }
}

#[async_trait]
impl AgentBuilder for AgentBuilderModule {
    fn policy(&self) -> BuilderPolicy {
        self.core().policy().clone()
    }

    fn build(&self, draft: &AgentDraft) -> Result<Built, BuildError> {
        self.core().build(draft)
    }

    fn preview(&self, manifest: &AgentManifest) -> Preview {
        self.core().preview(manifest)
    }

    async fn dry_run(
        &self,
        manifest: &AgentManifest,
        scenario: &DryScenario,
    ) -> Result<DryRunReport, BuildError> {
        let bus = self.bus()?;
        let (report, events) = self.core().dry_run(manifest, scenario)?;
        Self::publish(&bus, events).await;
        Ok(report)
    }

    /// Zapis: warunki (rdzeń) → rola i persona w `personas` → magazyn → rdzeń → zdarzenia.
    async fn save(
        &self,
        manifest: &AgentManifest,
        approval: BuilderApproval,
    ) -> Result<SavedAgent, BuildError> {
        let bus = self.bus()?;
        let built = self.core().prepare_save(manifest, &approval)?;
        let m = &built.manifest;
        self.personas
            .add_role(m.role.clone())
            .await
            .map_err(|e| BuildError::Store(e.to_string()))?;
        self.personas
            .add_persona(m.persona.clone())
            .await
            .map_err(|e| BuildError::Store(e.to_string()))?;
        self.store.save(m).map_err(BuildError::Store)?;
        // Bez odświeżania katalogu: persona i rola są już w `personas`, a rdzeń dopisuje je sam.
        let (saved, events) = self
            .core
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .commit(built)?;
        Self::publish(&bus, events).await;
        Ok(saved)
    }

    fn library(&self) -> Vec<AgentManifest> {
        self.core().library().to_vec()
    }
}

#[async_trait]
impl Module for AgentBuilderModule {
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
