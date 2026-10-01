//! Uruchamianie przez `agent-runtime`: umiejętność nie może przekroczyć roli wywołującej
//! (każde wymagane narzędzie dozwolone dla jej ról), przebieg dostaje kopertę uprawnień =
//! wymagania umiejętności ∩ koperta wywołującej (potomek ≤ rodzic), budżet ∩, pochodzenie,
//! taint i proweniencja wywołującej.

use agent_runtime_contract::{RunGrant, RunId, RunOptions, RunSpec, min_budget};
use personas_contract::Role;
use tools_common_contract::ToolManifest;

use crate::error::SkillError;
use crate::model::{Skill, SkillRecord, SkillState};
use crate::validate::render_goal;

/// Czy role wywołującej pozwalają na wszystkie narzędzia umiejętności (brak ról = brak narzędzi).
pub fn runnable_by(
    skill: &Skill,
    catalog: &[ToolManifest],
    roles: &[Role],
) -> Result<(), SkillError> {
    let groups: Vec<String> = roles.iter().flat_map(|r| r.tools.clone()).collect();
    let read_only = !roles.is_empty() && roles.iter().all(|r| r.read_only);
    for t in &skill.required_tools {
        let m = catalog
            .iter()
            .find(|m| &m.name == t)
            .ok_or_else(|| SkillError::UnknownTool(t.clone()))?;
        if roles.is_empty() || !m.allowed_for(&groups, read_only) {
            return Err(SkillError::ExceedsRole(t.clone()));
        }
    }
    Ok(())
}

/// Specyfikacja i opcje przebiegu umiejętności dla wywołującej (`caller` — jej przebieg albo
/// specyfikacja z obsady; `parent` — przebieg, z którego agentka uruchamia umiejętność).
pub fn prepare_run(
    record: &SkillRecord,
    params: &serde_json::Value,
    catalog: &[ToolManifest],
    caller: &RunSpec,
    caller_options: &RunOptions,
    parent: Option<RunId>,
) -> Result<(RunSpec, RunOptions), SkillError> {
    if record.state != SkillState::Installed {
        return Err(SkillError::WrongState(record.state));
    }
    let skill = &record.skill;
    runnable_by(skill, catalog, &caller.roles)?;
    let goal = render_goal(skill, params)?;
    let budget = match &skill.budget {
        Some(b) => min_budget(&caller.budget, b),
        None => caller.budget,
    };
    let wanted = RunGrant {
        tools: skill.required_tools.iter().cloned().collect(),
        capabilities: skill.required_capabilities.iter().cloned().collect(),
        read_only: !caller.roles.is_empty() && caller.roles.iter().all(|r| r.read_only),
        budget,
        max_autonomy: None,
    };
    let grant = match &caller_options.grant {
        Some(g) => g.attenuate(&wanted),
        None => wanted,
    };
    if let Some(lost) = skill
        .required_tools
        .iter()
        .find(|t| !grant.tools.contains(*t))
    {
        return Err(SkillError::ExceedsRole(lost.clone()));
    }
    let spec = RunSpec {
        session: caller.session.clone(),
        agent: caller.agent.clone(),
        persona: caller.persona.clone(),
        roles: caller.roles.clone(),
        goal,
        origin: caller.origin,
        model: caller.model.clone(),
        tools: skill.required_tools.clone(),
        budget: grant.budget,
        workdir: caller.workdir.clone(),
        verify: caller.verify,
        approval_timeout_ms: caller.approval_timeout_ms,
        history: Vec::new(),
    };
    let depth = caller_options.depth + u32::from(parent.is_some());
    let options = RunOptions {
        parent,
        depth,
        grant: Some(grant),
        crew: caller_options.crew.clone(),
        inherited_taint: caller_options.inherited_taint.clone(),
        trusted_context: caller_options.trusted_context.clone(),
        untrusted_context: caller_options.untrusted_context.clone(),
        label: Some(format!("umiejętność: {} v{}", skill.name, skill.version)),
    };
    Ok((spec, options))
}
