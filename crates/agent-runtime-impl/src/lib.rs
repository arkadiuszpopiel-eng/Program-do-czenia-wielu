//! `agent-runtime` v1 — implementacja (docs/modules/agent-runtime/SPEC.md, PLAN §9.1–9.2, §9.6).
//!
//! Pętla **plan → akcja → obserwacja → weryfikacja** na dowolnym `ModelProvider` (tool use
//! z neutralnego IR), narzędzia z rejestru (manifesty → `ToolSpec`, filtr ról, koperta
//! uprawnień). Każde narzędzie samo przechodzi przez Brokera; runtime nie żąda zdolności.
//! Budżety sprawdzane w każdym punkcie atomowym; checkpoint po każdej turze i przed akcjami;
//! anulowanie przez `CancellationToken`; zdarzenia `agent.*` dla UI/Replay; wyniki narzędzi
//! w prompcie jako dane niezaufane (delimitacja, taint, proweniencja celu).
//!
//! v1 (F5): wiele przebiegów równolegle (każdy z własną agentką, budżetem i taintem; zasoby
//! wyłączne przez `scheduler-lite` — [`RuntimeExt::locks`]), **granica kroku** przed każdym
//! wywołaniem narzędzia (sterowanie ≤ 1 krok atomowy, `StepGate::boundary` schedulera),
//! równoległe wywołania tylko do odczytu (zapisy szeregowo), **delegacja** do innej roli
//! z obsady (podprzebieg, koperta potomka ≤ rodzica, ta sama sesja, taint dziedziczony),
//! **Krytyczka** zamiast samoweryfikacji (osobna rola tylko do odczytu, ≠ autorka), raport
//! końcowy ([`AgentRuntime::report`]) i adapter [`RuntimeExecutor`] (`TaskExecutor`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod boundary;
mod child;
mod critic;
mod delegate;
mod engine;
mod executor;
mod flow;
mod handle;
mod locks;
mod prompt;
mod registry;
mod shared;
mod store;
mod tools;
mod turn;

use std::sync::Arc;

use agent_runtime_contract::{
    AgentRuntime, Checkpoint, CheckpointStore, RunBudget, RunError, RunEventEnvelope, RunId,
    RunOptions, RunOutcome, RunSpec, RunStatus, Steer,
};
use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use providers_contract::ModelProvider;
use tools_common_contract::Tool;

pub use delegate::{
    ChildPlan, DelegationError, MAX_CHILD_GOAL, MIN_CHILD_STEPS, ParentView, parent_grant,
    plan_delegation, remaining_budget,
};
pub use executor::{RuntimeExecutor, adapt_payload, task_run_id};
pub use locks::resources_for;
pub use shared::{AutonomyOracle, BrokerAutonomy, RuntimeExt};
pub use store::DirCheckpointStore;

use shared::{Hooks, Shared};

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
    /// v1: najgłębsza delegacja (podprzebieg podprzebiegu…).
    pub max_delegation_depth: u32,
    /// v1: najwięcej równoległych wywołań tylko do odczytu w jednej paczce.
    pub max_parallel_reads: usize,
    /// v1: najdłuższe czekanie na zasób wyłączny (ms) — potem wywołanie pominięte z powodem.
    pub lease_wait_ms: u64,
    /// v1: budżet jednej rundy Krytyczki (przycinany do reszty budżetu autorki).
    pub critic_budget: RunBudget,
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
            max_delegation_depth: 2,
            max_parallel_reads: 4,
            lease_wait_ms: 120_000,
            critic_budget: RunBudget {
                max_steps: 8,
                max_tokens: 60_000,
                max_wall_ms: 5 * 60 * 1000,
                max_cost_micro_usd: None,
                max_tool_calls_per_turn: 4,
            },
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

/// Runtime agentek (v1; zgodny wstecz z v0).
pub struct Runtime {
    shared: Arc<Shared>,
}

impl Runtime {
    /// Runtime nad zależnościami.
    pub fn new(deps: RuntimeDeps) -> Self {
        Self::with_ext(deps, RuntimeExt::default())
    }

    /// Runtime z zależnościami v1 (zasoby wyłączne, poziomy autonomii).
    pub fn with_ext(deps: RuntimeDeps, ext: RuntimeExt) -> Self {
        let shared = Shared::new(
            deps.provider,
            deps.tools,
            deps.checkpoints,
            deps.bus,
            deps.config,
        )
        .with_ext(ext);
        Self {
            shared: Arc::new(shared),
        }
    }

    pub(crate) fn shared(&self) -> Arc<Shared> {
        self.shared.clone()
    }

    fn launch(&self, cp: Checkpoint) -> Result<RunId, RunError> {
        self.shared
            .launch(cp, Hooks::default(), None)
            .map(|(h, _)| h.run.clone())
    }
}

#[async_trait]
impl AgentRuntime for Runtime {
    async fn start(&self, spec: RunSpec) -> Result<RunId, RunError> {
        self.start_with(spec, RunOptions::default()).await
    }

    async fn start_with(&self, spec: RunSpec, options: RunOptions) -> Result<RunId, RunError> {
        spec.validate().map_err(RunError::InvalidSpec)?;
        if let Some(grant) = &options.grant
            && !agent_runtime_contract::budget_within(&spec.budget, &grant.budget)
        {
            return Err(RunError::InvalidSpec(
                "budżet przebiegu przekracza sufit koperty uprawnień".into(),
            ));
        }
        let run = RunId::new(uuid::Uuid::new_v4().to_string());
        self.launch(Checkpoint::with_options(run, spec, options))
    }

    fn steer(&self, run: &RunId, steer: Steer) -> Result<(), RunError> {
        let h = self.shared.handle(run)?;
        if !h.is_active() {
            return Err(RunError::AlreadyFinished(run.clone()));
        }
        h.push_steer(steer, false);
        Ok(())
    }

    fn cancel(&self, run: &RunId) -> Result<(), RunError> {
        let h = self.shared.handle(run)?;
        h.cancel.cancel();
        h.wake.notify_one();
        Ok(())
    }

    async fn resume(&self, run: &RunId) -> Result<(), RunError> {
        if self.shared.find(run).is_some_and(|h| h.is_running()) {
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
        let mut rx = self.shared.handle(run)?.outcome_watch();
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
        Ok(self.shared.handle(run)?.status())
    }

    fn events(&self, run: &RunId) -> Result<Vec<RunEventEnvelope>, RunError> {
        Ok(self.shared.handle(run)?.events())
    }

    fn subscribe(
        &self,
        run: &RunId,
    ) -> Result<tokio::sync::broadcast::Receiver<RunEventEnvelope>, RunError> {
        Ok(self.shared.handle(run)?.subscribe())
    }

    fn children(&self, run: &RunId) -> Result<Vec<RunId>, RunError> {
        Ok(self.shared.handle(run)?.children())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "agent-runtime");
        let c = RuntimeConfig::default();
        assert_eq!(c.loop_max_repeats, 3);
        assert!(c.max_delegation_depth >= 1 && c.max_parallel_reads >= 2);
    }
}
