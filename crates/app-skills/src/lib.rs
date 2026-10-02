//! Umiejętności i Kreator agentów w aplikacji (kategoria `app-*`, wydzielona z `app-core` — limit
//! rozmiaru crate'a):
//! - [`SkillsApp`] — `skills-impl` (`DirSkillStore` w `%LOCALAPPDATA%\Alfa\skills`): biblioteka,
//!   przegląd propozycji (diff + hash), instalacja i zwolnienie z kwarantanny wyłącznie z UI,
//!   uruchomienie jako zadanie agentki z kopertą `prepare_run`, eksport/import paczki;
//! - [`BuilderApp`] — `agent-builder-impl` (`DirManifestStore` w `%LOCALAPPDATA%\Alfa\agents`,
//!   sufit autonomii z Brokera): szkic z rozmowy/formularza → podgląd persony → test na sucho →
//!   zapis nie-głosem;
//! - [`open`] — budowa i start obu modułów (błąd budowy = moduł niezdrowy, komendy „niedostępne").

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod builder;
mod diff;
mod skills;

use std::path::Path;
use std::sync::Arc;

use agent_builder_impl::{AgentBuilderModule, CeilingSource, DirManifestStore, FixedCeiling};
use app_api::EventHub;
use app_api::ports::{ShellPort, VoicePort};
use app_tasks::TasksApp;
use core_bus_contract::{EventBus, SessionId};
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use personas_contract::Personas;
use risk_classifier_contract::AutonomyLevel;
use safety_broker_contract::Broker;
use skills_impl::{DirSkillStore, SkillsModule};
use tools_common_contract::ToolManifest;

pub use builder::{BuilderApp, default_scenario};
pub use diff::diff_lines;
pub use skills::{SkillsApp, info as skill_info, spawn_bridge};

/// Manifesty modułów składanych przez ten crate (identyfikator → `module.toml`).
pub const MODULES: &[(&str, &str)] = &[
    ("skills", skills_impl::MODULE_TOML),
    ("agent-builder", agent_builder_impl::MODULE_TOML),
];

/// Sesja-sonda poziomu globalnego Brokera (identyfikatory sesji to UUIDv7 — brak kolizji).
const GLOBAL_PROBE: &str = "__alfa_global__";

/// Sufit autonomii nowej agentki: poziom globalny z Brokera (Kreator i tak przycina do L3).
struct BrokerCeiling(Arc<dyn Broker>);

impl CeilingSource for BrokerCeiling {
    fn ceiling(&self) -> AutonomyLevel {
        self.0.autonomy(&SessionId::new(GLOBAL_PROBE), None)
    }
}

/// Zależności umiejętności i Kreatora.
pub struct WorkDeps {
    /// Katalog narzędzi agentek (manifesty — walidacja przepisów i ról).
    pub catalog: Vec<ToolManifest>,
    /// Dane lokalne (`%LOCALAPPDATA%\Alfa`).
    pub local: std::path::PathBuf,
    /// Magistrala (moduły publikują `skills.*` i `agent_builder.*`).
    pub bus: Arc<dyn EventBus>,
    /// Zadania (uruchomienie umiejętności).
    pub tasks: Arc<TasksApp>,
    /// Powłoka (dialogi eksportu/importu).
    pub shell: Arc<dyn ShellPort>,
    /// Persony (zapis agentki Kreatorem).
    pub personas: Arc<dyn Personas>,
    /// Broker (sufit autonomii; bez niego — L2).
    pub broker: Option<Arc<dyn Broker>>,
    /// Głos (odsłuch).
    pub voice: Arc<dyn VoicePort>,
}

async fn start<M: Module>(
    mut module: M,
    id: &str,
    bus: &Arc<dyn EventBus>,
) -> Result<Arc<M>, String> {
    let id = core_registry_contract::ModuleId::new(id).map_err(|e| e.to_string())?;
    module
        .start(ModuleContext::new(id, bus.clone()))
        .await
        .map_err(|e| e.to_string())?;
    Ok(Arc::new(module))
}

fn dir(local: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    let d = local.join(name);
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    Ok(d)
}

/// Umiejętności i Kreator gotowe do komend.
#[derive(Clone)]
pub struct Work {
    /// Umiejętności.
    pub skills: Arc<SkillsApp>,
    /// Kreator.
    pub builder: Arc<BuilderApp>,
}

impl Work {
    /// Zdrowie modułu `skills` / `agent-builder` (rejestr).
    pub fn health(&self, id: &str) -> HealthStatus {
        let of = |r: Result<HealthStatus, String>| r.unwrap_or_else(HealthStatus::Unhealthy);
        match id {
            "skills" => of(self.skills.module().map(|m| m.health())),
            "agent-builder" => of(self.builder.module().map(|m| m.health())),
            _ => HealthStatus::NotStarted,
        }
    }
}

/// Buduje i uruchamia moduły (błąd jednego nie zatrzymuje drugiego ani aplikacji).
pub async fn open(d: WorkDeps) -> Work {
    let skills = async {
        let store = DirSkillStore::open(dir(&d.local, "skills")?)?;
        let module = SkillsModule::new(d.catalog.clone(), Arc::new(store))?;
        start(module, "skills", &d.bus).await
    }
    .await;
    let builder = async {
        let store = DirManifestStore::open(dir(&d.local, "agents")?)?;
        let ceiling: Arc<dyn CeilingSource> = match &d.broker {
            Some(b) => Arc::new(BrokerCeiling(b.clone())),
            None => Arc::new(FixedCeiling(AutonomyLevel::L2)),
        };
        let module = AgentBuilderModule::new(
            d.personas.clone(),
            d.catalog.clone(),
            Arc::new(store),
            ceiling,
        )?;
        start(module, "agent-builder", &d.bus).await
    }
    .await;
    Work {
        skills: Arc::new(SkillsApp::new(skills, d.tasks, d.shell)),
        builder: Arc::new(BuilderApp::new(builder, d.catalog, d.voice)),
    }
}

/// Zdarzenia `skills.*` → UI (`SkillsChanged`).
pub async fn bridge(bus: Arc<dyn EventBus>, events: EventHub) {
    spawn_bridge(bus, events).await;
}
