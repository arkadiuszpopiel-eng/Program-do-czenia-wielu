//! Start przebiegów `agent-runtime` v1 w aplikacji: `Runtime::with_ext` (zasoby wyłączne
//! schedulera, poziomy autonomii z Brokera), `start_with` z obsadą sesji (`Crew` → delegacja
//! i Krytyczka jako podprzebiegi) i — dla umiejętności — koperta `prepare_run` (≤ wywołującej).
//! Checkpointy w pamięci procesu; ten sam magazyn czyta [`crate::RunFamily`] (agentka podprzebiegu).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use agent_runtime_contract::{
    AgentRuntime, Crew, MemCheckpointStore, RunError, RunId, RunOptions, RunSpec,
};
use agent_runtime_impl::{BrokerAutonomy, Runtime, RuntimeConfig, RuntimeDeps, RuntimeExt};
use core_bus_contract::{EventBus, SessionId};
use personas_contract::Personas;
use providers_contract::ModelProvider;
use safety_broker_contract::Broker;
use skills_contract::{SkillId, Skills};
use tools_common_contract::Tool;

/// Umiejętność do uruchomienia w przebiegu (ładunek zadania `skill`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillCall {
    /// Identyfikator umiejętności (zainstalowanej).
    pub id: String,
    /// Parametry (walidowane schematem umiejętności).
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Zależności startu przebiegów v1 (wspólne dla czatu i zadań).
#[derive(Clone, Default)]
pub struct Launch {
    ext: RuntimeExt,
    personas: Option<Arc<dyn Personas>>,
    skills: Arc<OnceLock<Arc<dyn Skills>>>,
}

impl std::fmt::Debug for Launch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Launch")
            .field("locks", &self.ext.locks.is_some())
            .field("autonomy", &self.ext.autonomy.is_some())
            .field("crew", &self.personas.is_some())
            .field("skills", &self.skills.get().is_some())
            .finish()
    }
}

impl Launch {
    /// Zasoby wyłączne (`scheduler`), poziomy autonomii (Broker) i obsada (`personas`).
    pub fn new(
        locks: Option<Arc<dyn scheduler_contract::SchedulerLite>>,
        broker: Option<Arc<dyn Broker>>,
        personas: Option<Arc<dyn Personas>>,
    ) -> Self {
        let autonomy = broker
            .map(|b| Arc::new(BrokerAutonomy(b)) as Arc<dyn agent_runtime_impl::AutonomyOracle>);
        Self {
            ext: RuntimeExt { locks, autonomy },
            personas,
            skills: Arc::default(),
        }
    }

    /// Biblioteka umiejętności (wiązana po złożeniu modułów; drugi raz — bez zmiany).
    pub fn bind_skills(&self, skills: Arc<dyn Skills>) {
        let _ = self.skills.set(skills);
    }

    /// Obsada sesji dla delegacji i Krytyczki (`None` — bez usługi person).
    pub fn crew(&self, session: &SessionId) -> Option<Crew> {
        let personas = self.personas.as_ref()?;
        Some(Crew {
            cast: personas.cast(session),
            personas: personas.personas(),
            roles: personas.roles(),
            models: BTreeMap::new(),
        })
    }

    /// Opcje przebiegu głównego: obsada + etykieta.
    pub fn options(&self, session: &SessionId, label: Option<String>) -> RunOptions {
        RunOptions {
            crew: self.crew(session),
            label,
            ..RunOptions::default()
        }
    }

    /// Runtime v1 nad dostawcą i narzędziami (+ magazyn checkpointów dla [`crate::RunFamily`]).
    pub fn runtime(
        &self,
        provider: Arc<dyn ModelProvider>,
        tools: Vec<Arc<dyn Tool>>,
        bus: Option<Arc<dyn EventBus>>,
    ) -> (Arc<Runtime>, MemCheckpointStore) {
        let store = MemCheckpointStore::default();
        let runtime = Runtime::with_ext(
            RuntimeDeps {
                provider,
                tools,
                checkpoints: Arc::new(store.clone()),
                bus,
                config: RuntimeConfig::default(),
            },
            self.ext.clone(),
        );
        (Arc::new(runtime), store)
    }

    /// Przebieg umiejętności: koperta = wymagania ∩ wywołująca (`prepare_run`), obsada zostaje.
    pub fn skill_run(
        &self,
        call: &SkillCall,
        caller: &RunSpec,
        caller_options: &RunOptions,
    ) -> Result<(RunSpec, RunOptions), String> {
        let skills = self
            .skills
            .get()
            .ok_or("biblioteka umiejętności niepodłączona")?;
        let (spec, mut options) = skills
            .prepare_run(
                &SkillId::new(call.id.as_str()),
                &call.params,
                caller,
                caller_options,
                None,
            )
            .map_err(|e| e.to_string())?;
        if options.crew.is_none() {
            options.crew.clone_from(&caller_options.crew);
        }
        Ok((spec, options))
    }

    /// Start przebiegu głównego przez `start_with`.
    pub async fn start(
        runtime: &Runtime,
        spec: RunSpec,
        options: RunOptions,
    ) -> Result<RunId, RunError> {
        runtime.start_with(spec, options).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_without_services_has_no_crew_and_no_skills() {
        let launch = Launch::default();
        assert!(launch.crew(&SessionId::new("s")).is_none());
        let options = launch.options(&SessionId::new("s"), Some("x".into()));
        assert_eq!(options.label.as_deref(), Some("x"));
        assert!(options.crew.is_none());
        assert!(format!("{launch:?}").contains("skills: false"));
    }
}
