//! `agent-runtime` v0 — implementacja (docs/modules/agent-runtime/SPEC.md, PLAN §9.1, §9.6).
//!
//! Jedna agentka, pętla **plan → akcja → obserwacja → weryfikacja** na dowolnym
//! `ModelProvider` (tool use z neutralnego IR), narzędzia sekwencyjnie z rejestru (manifesty →
//! `ToolSpec`, filtr ról). Każde narzędzie samo przechodzi przez Brokera; runtime nie żąda
//! zdolności. Budżety (kroki, tokeny, czas, koszt) sprawdzane w każdym punkcie atomowym;
//! checkpoint po każdej turze i przed akcjami (wznowienie po restarcie bez ponownego wykonania
//! przerwanych wywołań); anulowanie przez `CancellationToken` (model, czekanie na zgodę,
//! proces powłoki); steering przyjmowany w następnym kroku; zdarzenia `agent.*` dla UI/Replay;
//! wyniki narzędzi w prompcie jako dane niezaufane (delimitacja, taint, proweniencja celu).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod engine;
mod flow;
mod handle;
mod prompt;
mod registry;
mod store;
mod turn;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use agent_runtime_contract::{
    AgentRuntime, Checkpoint, CheckpointStore, RunError, RunEventEnvelope, RunId, RunOutcome,
    RunSpec, RunStatus, Steer,
};
use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use providers_contract::ModelProvider;
use tools_common_contract::Tool;

pub use store::DirCheckpointStore;

use engine::{Engine, Shared};
use handle::RunHandle;
use registry::ToolRegistry;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Konfiguracja (`[agent]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Detektor pętli: tyle identycznych wywołań w oknie = stop (`loop_detector.max_repeats`).
    pub loop_max_repeats: u32,
    /// Okno detektora pętli (ostatnie wywołania).
    pub loop_window: usize,
    /// Limit tokenów wyjścia jednej tury.
    pub max_output_tokens: Option<u32>,
    /// Maksymalna długość wyniku narzędzia w prompcie (znaki).
    pub tool_result_max_chars: usize,
    /// Narzędzia zwracające metadane (nazwy plików) — nie zasilają proweniencji celu.
    pub metadata_tools: Vec<String>,
    /// Limit buforów proweniencji (B).
    pub provenance_cap: usize,
    /// Maksymalna liczba rund weryfikacji z poprawkami.
    pub max_verify_rounds: u32,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            loop_max_repeats: 3,
            loop_window: 12,
            max_output_tokens: Some(4096),
            tool_result_max_chars: 24_000,
            metadata_tools: vec!["fs_list".into(), "fs_stat".into(), "fs_search".into()],
            provenance_cap: 256 * 1024,
            max_verify_rounds: 2,
        }
    }
}

/// Zależności runtime.
#[derive(Clone)]
pub struct RuntimeDeps {
    /// Dostawca modeli (zwykle Router).
    pub provider: Arc<dyn ModelProvider>,
    /// Wszystkie narzędzia (zestawy `tools-*`).
    pub tools: Vec<Arc<dyn Tool>>,
    /// Checkpointy.
    pub checkpoints: Arc<dyn CheckpointStore>,
    /// Magistrala (zdarzenia `agent.*`).
    pub bus: Option<Arc<dyn EventBus>>,
    /// Konfiguracja.
    pub config: RuntimeConfig,
}

/// Runtime agentki v0.
pub struct Runtime {
    shared: Arc<Shared>,
    bus: Option<Arc<dyn EventBus>>,
    runs: Mutex<BTreeMap<RunId, Arc<RunHandle>>>,
}

impl Runtime {
    /// Runtime nad zależnościami.
    pub fn new(deps: RuntimeDeps) -> Self {
        Self {
            shared: Arc::new(Shared {
                provider: deps.provider,
                tools: deps.tools,
                store: deps.checkpoints,
                config: deps.config,
            }),
            bus: deps.bus,
            runs: Mutex::new(BTreeMap::new()),
        }
    }

    fn runs(&self) -> MutexGuard<'_, BTreeMap<RunId, Arc<RunHandle>>> {
        self.runs.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn handle(&self, run: &RunId) -> Result<Arc<RunHandle>, RunError> {
        self.runs()
            .get(run)
            .cloned()
            .ok_or_else(|| RunError::UnknownRun(run.clone()))
    }

    fn launch(&self, cp: Checkpoint) -> Result<RunId, RunError> {
        let registry =
            ToolRegistry::for_spec(&self.shared.tools, &cp.spec).map_err(RunError::InvalidSpec)?;
        let run = cp.run.clone();
        let handle = Arc::new(RunHandle::new(
            run.clone(),
            cp.spec.session.clone(),
            cp.spec.agent.clone(),
            self.bus.clone(),
        ));
        self.runs().insert(run.clone(), handle.clone());
        let engine = Engine::new(self.shared.clone(), handle, cp, registry);
        tokio::spawn(engine.run());
        Ok(run)
    }
}

#[async_trait]
impl AgentRuntime for Runtime {
    async fn start(&self, spec: RunSpec) -> Result<RunId, RunError> {
        spec.validate().map_err(RunError::InvalidSpec)?;
        let run = RunId::new(uuid::Uuid::new_v4().to_string());
        self.launch(Checkpoint::initial(run, spec))
    }

    fn steer(&self, run: &RunId, steer: Steer) -> Result<(), RunError> {
        let h = self.handle(run)?;
        if !h.is_active() {
            return Err(RunError::AlreadyFinished(run.clone()));
        }
        h.push_steer(steer);
        Ok(())
    }

    fn cancel(&self, run: &RunId) -> Result<(), RunError> {
        let h = self.handle(run)?;
        h.cancel.cancel();
        h.wake.notify_one();
        Ok(())
    }

    async fn resume(&self, run: &RunId) -> Result<(), RunError> {
        if self.runs().get(run).is_some_and(|h| h.is_active()) {
            return Err(RunError::AlreadyRunning(run.clone()));
        }
        let cp = self
            .shared
            .store
            .latest(run)
            .map_err(|e| RunError::Store(e.to_string()))?
            .ok_or_else(|| RunError::NoCheckpoint(run.clone()))?;
        if cp.finished.is_some() {
            return Err(RunError::AlreadyFinished(run.clone()));
        }
        self.launch(cp).map(|_| ())
    }

    async fn wait(&self, run: &RunId) -> Result<RunOutcome, RunError> {
        let mut rx = self.handle(run)?.outcome_watch();
        loop {
            if let Some(o) = rx.borrow_and_update().clone() {
                return Ok(o);
            }
            if rx.changed().await.is_err() {
                return Err(RunError::UnknownRun(run.clone()));
            }
        }
    }

    fn status(&self, run: &RunId) -> Result<RunStatus, RunError> {
        Ok(self.handle(run)?.status())
    }

    fn events(&self, run: &RunId) -> Result<Vec<RunEventEnvelope>, RunError> {
        Ok(self.handle(run)?.events())
    }

    fn subscribe(
        &self,
        run: &RunId,
    ) -> Result<tokio::sync::broadcast::Receiver<RunEventEnvelope>, RunError> {
        Ok(self.handle(run)?.subscribe())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "agent-runtime");
        assert_eq!(RuntimeConfig::default().loop_max_repeats, 3);
    }
}
