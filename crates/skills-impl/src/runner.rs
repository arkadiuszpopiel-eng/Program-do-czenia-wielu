//! Uruchamianie umiejętności przez `agent-runtime` (port `AgentRuntime`): przebieg z kopertą
//! uprawnień = wymagania umiejętności ∩ koperta wywołującej, z etykietą dla Replay; podpowiedź
//! umiejętności do zadania (deterministycznie, opcjonalnie z modelem).

use std::sync::Arc;

use agent_runtime_contract::{AgentRuntime, RunId, RunOptions, RunSpec};
use personas_contract::Role;
use skills_contract::{SkillError, SkillId, SkillMatch, SkillRanker, Skills, rerank};

/// Uruchamiacz umiejętności.
pub struct SkillRunner {
    skills: Arc<dyn Skills>,
    runtime: Arc<dyn AgentRuntime>,
}

impl SkillRunner {
    /// Uruchamiacz nad biblioteką i runtime.
    pub fn new(skills: Arc<dyn Skills>, runtime: Arc<dyn AgentRuntime>) -> Self {
        Self { skills, runtime }
    }

    /// Startuje przebieg umiejętności dla wywołującej (`parent` = przebieg agentki, która ją
    /// uruchamia). Błędy uprawnień i parametrów — zanim cokolwiek wystartuje.
    pub async fn run(
        &self,
        id: &SkillId,
        params: &serde_json::Value,
        caller: &RunSpec,
        caller_options: &RunOptions,
        parent: Option<RunId>,
    ) -> Result<RunId, SkillError> {
        let (spec, options) =
            self.skills
                .prepare_run(id, params, caller, caller_options, parent)?;
        self.runtime
            .start_with(spec, options)
            .await
            .map_err(|e| SkillError::Invalid(e.to_string()))
    }

    /// Umiejętności pasujące do zadania (dozwolone dla ról); z modelem — przestawione przez niego.
    pub async fn suggest(
        &self,
        task: &str,
        roles: &[Role],
        ranker: Option<&dyn SkillRanker>,
        limit: usize,
    ) -> Vec<SkillMatch> {
        let found = self.skills.search(task, roles, limit.max(1));
        match ranker {
            Some(r) => {
                let mut out = rerank(r, task, found).await;
                out.truncate(limit);
                out
            }
            None => found,
        }
    }
}
