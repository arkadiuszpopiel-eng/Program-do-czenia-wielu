//! Atrapa Kreatora agentów (docs/modules/agent-builder/SPEC.md, „Fake”): rdzeń
//! [`BuilderCore`] z kontraktu nad wbudowanym katalogiem person i głosami v0, biblioteka
//! w pamięci, zdarzenia nagrywane zamiast magistrali. Do testów UI i `app-*` — wyłącznie jako
//! dev-dependency.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Mutex, MutexGuard};

use agent_builder_contract::{
    AgentBuilder, AgentDraft, AgentManifest, BuildError, BuilderApproval, BuilderCore,
    BuilderPolicy, Built, DryRunReport, DryScenario, Preview, SavedAgent,
};
use async_trait::async_trait;
use core_bus_contract::Event;
use personas_contract::Catalog;
use tools_common_contract::ToolManifest;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Atrapa Kreatora.
pub struct FakeAgentBuilder {
    core: Mutex<BuilderCore>,
    events: Mutex<Vec<Event>>,
}

impl FakeAgentBuilder {
    /// Kreator z polityką i katalogiem narzędzi (persony wbudowane, głosy v0).
    pub fn new(policy: BuilderPolicy, tools: Vec<ToolManifest>) -> Self {
        Self {
            core: Mutex::new(BuilderCore::new(
                policy,
                tools,
                Catalog::builtin(),
                voice_tts_contract::v0_chains(),
            )),
            events: Mutex::new(Vec::new()),
        }
    }

    /// Nagrane zdarzenia.
    pub fn events(&self) -> Vec<Event> {
        lock(&self.events).clone()
    }
}

#[async_trait]
impl AgentBuilder for FakeAgentBuilder {
    fn policy(&self) -> BuilderPolicy {
        lock(&self.core).policy().clone()
    }

    fn build(&self, draft: &AgentDraft) -> Result<Built, BuildError> {
        lock(&self.core).build(draft)
    }

    fn preview(&self, manifest: &AgentManifest) -> Preview {
        lock(&self.core).preview(manifest)
    }

    async fn dry_run(
        &self,
        manifest: &AgentManifest,
        scenario: &DryScenario,
    ) -> Result<DryRunReport, BuildError> {
        let (report, ev) = lock(&self.core).dry_run(manifest, scenario)?;
        lock(&self.events).extend(ev);
        Ok(report)
    }

    async fn save(
        &self,
        manifest: &AgentManifest,
        approval: BuilderApproval,
    ) -> Result<SavedAgent, BuildError> {
        let mut core = lock(&self.core);
        let built = core.prepare_save(manifest, &approval)?;
        let (saved, ev) = core.commit(built)?;
        drop(core);
        lock(&self.events).extend(ev);
        Ok(saved)
    }

    fn library(&self) -> Vec<AgentManifest> {
        lock(&self.core).library().to_vec()
    }
}
