//! Delegacja (PLAN §9.2, §9.7: fan-out) — podprzebieg innej roli z obsady w tej samej sesji.
//! Plan potomka ([`plan_delegation`]) jest czystą funkcją: koperta uprawnień potomka powstaje
//! przez atenuację koperty rodzica (**potomek ≤ rodzic**: narzędzia, rodziny zdolności, tylko
//! odczyt, budżet = reszta budżetu rodzica, sufit autonomii), pochodzenie polecenia i taint są
//! dziedziczone, a agentka musi grać wskazaną rolę w obsadzie.

use std::collections::BTreeSet;

use agent_runtime_contract::{
    Crew, DELEGATE_GROUP, DELEGATE_TOOL, DelegateArgs, RunBudget, RunGrant, RunId, RunOptions,
    RunSpec, min_budget,
};
use personas_contract::{PersonaId, RoleId};
use risk_classifier_contract::AutonomyLevel;
use safety_broker_contract::TaintSource;
use tools_common_contract::ToolManifest;

/// Najdłuższy cel podzadania (znaki).
pub const MAX_CHILD_GOAL: usize = 4000;
/// Najmniejsza liczba kroków, z jaką podzadanie ma sens.
pub const MIN_CHILD_STEPS: u32 = 2;

/// Widok rodzica potrzebny do planu delegacji.
#[derive(Debug, Clone)]
pub struct ParentView<'a> {
    /// Przebieg-rodzic.
    pub run: &'a RunId,
    /// Specyfikacja rodzica.
    pub spec: &'a RunSpec,
    /// Opcje rodzica (obsada, koperta, głębokość).
    pub options: &'a RunOptions,
    /// Koperta rodzica: manifesty narzędzi przebiegu (`spec.tools` ∩ koperta v1, przed filtrem
    /// ról zlecającej — Dyrygentka sama nie pisze plików, ale może zlecić to Wykonawczyni).
    pub tools: Vec<ToolManifest>,
    /// Reszta budżetu rodzica.
    pub remaining: RunBudget,
    /// Taint rodzica.
    pub taint: Option<TaintSource>,
    /// Poziom autonomii rodzica i wykonawczyni (z Brokera; `None` = nieznany).
    pub autonomy: (Option<AutonomyLevel>, Option<AutonomyLevel>),
    /// Treść zaufana i niezaufana rodzica (proweniencja).
    pub provenance: (&'a str, &'a str),
}

/// Plan podprzebiegu.
#[derive(Debug, Clone, PartialEq)]
pub struct ChildPlan {
    /// Specyfikacja potomka.
    pub spec: RunSpec,
    /// Opcje potomka (koperta ⊆ koperta rodzica).
    pub options: RunOptions,
}

/// Dlaczego delegacja odrzucona.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DelegationError {
    /// Brak obsady w opcjach przebiegu.
    #[error("brak obsady — delegacja niedostępna")]
    NoCrew,
    /// Za głęboko.
    #[error("przekroczona głębokość delegacji ({0})")]
    Depth(u32),
    /// Pusty albo za długi cel.
    #[error("cel podzadania pusty albo za długi")]
    Goal,
    /// Nieznana rola.
    #[error("nieznana rola `{0}`")]
    UnknownRole(String),
    /// Nikt (albo wskazana agentka) nie gra tej roli w obsadzie.
    #[error("w obsadzie nikt taki nie gra roli `{0}`")]
    NotInCast(String),
    /// Narzędzie spoza uprawnień rodzica albo roli.
    #[error("narzędzie `{0}` wykracza poza Twoje uprawnienia albo uprawnienia roli")]
    ToolNotGranted(String),
    /// Za mało budżetu.
    #[error("za mały pozostały budżet na podzadanie")]
    Budget,
    /// Wykonawczyni ma wyższy poziom autonomii niż zlecająca.
    #[error("agentka ma wyższy poziom autonomii ({child:?}) niż zlecająca ({parent:?})")]
    Autonomy {
        /// Poziom zlecającej.
        parent: AutonomyLevel,
        /// Poziom wykonawczyni.
        child: AutonomyLevel,
    },
}

/// Koperta rodzica: jawna z opcji albo wyprowadzona z jego narzędzi i budżetu.
pub fn parent_grant(view: &ParentView<'_>) -> RunGrant {
    let derived = RunGrant {
        tools: view
            .tools
            .iter()
            .map(|m| m.name.clone())
            .chain(std::iter::once(DELEGATE_TOOL.to_owned()))
            .collect(),
        capabilities: view
            .tools
            .iter()
            .flat_map(|m| m.capabilities.iter().cloned())
            .collect(),
        read_only: !view.spec.roles.is_empty() && view.spec.roles.iter().all(|r| r.read_only),
        budget: view.remaining,
        max_autonomy: view.autonomy.0,
    };
    match &view.options.grant {
        Some(g) => g.attenuate(&derived),
        None => derived,
    }
}

/// Wykonawczyni podzadania: wskazana agentka (identyfikator przycięty) albo pierwsza z obsady
/// grająca rolę, inna niż zlecająca. Jedyne miejsce wyboru — ta sama agentka trafia do planu
/// i do zapytania o jej poziom autonomii (przegląd #2, SR2-01).
pub fn delegation_target(
    crew: &Crew,
    args: &DelegateArgs,
    parent_agent: &str,
) -> Option<PersonaId> {
    let holders = crew.cast.holders(&RoleId::new(args.role.trim()));
    match &args.persona {
        Some(p) => holders.into_iter().find(|h| h.as_str() == p.trim()),
        None => holders
            .iter()
            .find(|h| h.as_str() != parent_agent)
            .or(holders.first())
            .cloned(),
    }
}

/// Plan potomka: walidacja argumentów, wybór agentki z obsady, atenuacja koperty.
pub fn plan_delegation(
    view: &ParentView<'_>,
    args: &DelegateArgs,
    max_depth: u32,
) -> Result<ChildPlan, DelegationError> {
    let crew = view.options.crew.as_ref().ok_or(DelegationError::NoCrew)?;
    let depth = view.options.depth.saturating_add(1);
    if depth > max_depth {
        return Err(DelegationError::Depth(max_depth));
    }
    let goal = args.goal.trim();
    if goal.is_empty() || goal.chars().count() > MAX_CHILD_GOAL {
        return Err(DelegationError::Goal);
    }
    let role_id = RoleId::new(args.role.trim());
    let role = crew
        .role(&role_id)
        .ok_or_else(|| DelegationError::UnknownRole(args.role.clone()))?;
    let persona_id = delegation_target(crew, args, view.spec.agent.as_str())
        .ok_or_else(|| DelegationError::NotInCast(args.role.clone()))?;
    let persona = crew
        .persona(&persona_id)
        .ok_or_else(|| DelegationError::NotInCast(args.role.clone()))?;
    if let (Some(parent), Some(child)) = view.autonomy
        && child > parent
    {
        return Err(DelegationError::Autonomy { parent, child });
    }
    let parent = parent_grant(view);
    if let Some(wanted) = &args.tools
        && let Some(bad) = wanted.iter().find(|t| !parent.tools.contains(*t))
    {
        return Err(DelegationError::ToolNotGranted(bad.clone()));
    }
    let role_tools: Vec<&ToolManifest> = view
        .tools
        .iter()
        .filter(|m| m.allowed_for(&role.tools, role.read_only))
        .filter(|m| args.tools.as_ref().is_none_or(|w| w.contains(&m.name)))
        .collect();
    let mut tools: BTreeSet<String> = role_tools.iter().map(|m| m.name.clone()).collect();
    if role.tools.iter().any(|g| g == DELEGATE_GROUP) && !role.read_only {
        tools.insert(DELEGATE_TOOL.to_owned());
    }
    let mut budget = view.remaining;
    if let Some(steps) = args.max_steps {
        budget.max_steps = budget.max_steps.min(steps);
    }
    let request = RunGrant {
        tools,
        capabilities: role_tools
            .iter()
            .flat_map(|m| m.capabilities.iter().cloned())
            .collect(),
        read_only: role.read_only,
        budget,
        max_autonomy: view.autonomy.1,
    };
    let grant = parent.attenuate(&request);
    if grant.budget.max_steps < MIN_CHILD_STEPS || grant.budget.max_tokens == 0 {
        return Err(DelegationError::Budget);
    }
    let model = crew
        .models
        .get(&role.id)
        .cloned()
        .unwrap_or_else(|| view.spec.model.clone());
    let spec = RunSpec {
        session: view.spec.session.clone(),
        agent: core_bus_contract::AgentId::new(persona.id.as_str()),
        persona: persona.clone(),
        roles: vec![role.clone()],
        goal: goal.to_owned(),
        origin: view.spec.origin,
        model,
        tools: grant
            .tools
            .iter()
            .filter(|t| t.as_str() != DELEGATE_TOOL)
            .cloned()
            .collect(),
        budget: grant.budget,
        workdir: view.spec.workdir.clone(),
        verify: false,
        approval_timeout_ms: view.spec.approval_timeout_ms,
        history: Vec::new(),
    };
    let options = RunOptions {
        parent: Some(view.run.clone()),
        depth,
        grant: Some(grant),
        crew: Some(crew.clone()),
        inherited_taint: view.taint.clone(),
        trusted_context: view.provenance.0.to_owned(),
        untrusted_context: view.provenance.1.to_owned(),
        label: Some(format!(
            "delegacja: {} → {} ({})",
            view.spec.persona.name, persona.name, role.name
        )),
    };
    Ok(ChildPlan { spec, options })
}

/// Pozostały budżet (budżet rodzica minus zużycie własne i podprzebiegów).
pub fn remaining_budget(
    budget: &RunBudget,
    used: &agent_runtime_contract::UsageTotals,
    elapsed_ms: u64,
) -> RunBudget {
    let left = RunBudget {
        max_steps: budget.max_steps.saturating_sub(used.steps),
        max_tokens: budget.max_tokens.saturating_sub(used.tokens()),
        max_wall_ms: budget.max_wall_ms.saturating_sub(elapsed_ms),
        max_cost_micro_usd: budget
            .max_cost_micro_usd
            .map(|m| m.saturating_sub(used.cost_nano_usd / 1000)),
        max_tool_calls_per_turn: budget.max_tool_calls_per_turn,
    };
    min_budget(budget, &left)
}
