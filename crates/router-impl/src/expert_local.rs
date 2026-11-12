//! Dopuszczalność lokalnego modelu do kodu/trudnej pracy, niezależna od rankingu.
//! Lista identyfikuje wybrane profile, nie poświadcza jakości lub odbioru sprzętu.
use crate::routing::{Registered, RouterCore};
use providers_contract::{ChatRequest, Difficulty, Signal, estimate_request, estimate_text};
use router_contract::{Candidate, Constraints, RouteKind, TaskClass, class_from_policy};

const EXPERT_MODELS: [&str; 2] = ["qwen3.8-27b-ud-q4-k-m", "qwen3.8-27b-q4-0"];
pub(crate) const REQUIREMENT: &str = "kod lub trudne zadanie wymaga lokalnego profilu Qwen3.8-27B Q4 ze znanymi możliwościami albo dopuszczonej chmury; słabszy model lokalny nie jest trasą zapasową";

pub(crate) fn permitted(reg: &Registered, model: &str) -> bool {
    reg.kind == RouteKind::Local
        && EXPERT_MODELS.contains(&model)
        && reg.provider.capabilities().models.contains_key(model)
}

pub(crate) fn required(
    core: &RouterCore,
    class: TaskClass,
    constraints: &Constraints,
    request: Option<&ChatRequest>,
) -> bool {
    let hinted = constraints
        .hint
        .class
        .as_deref()
        .and_then(class_from_policy);
    if matches!(class, TaskClass::Code | TaskClass::Planning)
        || matches!(hinted, Some(TaskClass::Code | TaskClass::Planning))
    {
        return true;
    }
    let class = hinted.unwrap_or(class);
    if !matches!(class, TaskClass::Conversation | TaskClass::GuiVision) {
        return false;
    }
    if constraints.hint.difficulty == Some(Difficulty::Hard)
        || constraints
            .hint
            .escalate_from
            .as_deref()
            .and_then(Candidate::parse)
            .is_some_and(|c| EXPERT_MODELS.contains(&c.model.as_str()))
    {
        return true;
    }
    // Sam kod w zapytaniu badawczym jest tematem wyszukiwania; trudność nadal obowiązuje.
    let research = constraints.hint.class.as_deref() == Some("research");
    let strategy = core.difficulty_policy().strategy;
    let qualifies = |e: providers_contract::Estimate| {
        (!research
            && e.signals
                .iter()
                .any(|s| matches!(s, Signal::Code | Signal::CodeContext)))
            || e.level(strategy) == Difficulty::Hard
    };
    constraints
        .hint
        .task
        .as_deref()
        .is_some_and(|s| qualifies(estimate_text(s, false, false)))
        || request.is_some_and(|r| qualifies(estimate_request(r)))
}

/// Odkrycie znanego profilu nie instaluje modelu; dostępność sprawdza zwykły adapter.
pub(crate) fn candidates(core: &RouterCore) -> Vec<Candidate> {
    core.all_registered()
        .into_iter()
        .flat_map(|(id, reg)| {
            EXPERT_MODELS
                .iter()
                .filter(|m| permitted(&reg, m))
                .map(|m| Candidate::new(id.clone(), *m))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub(crate) fn prefer(core: &RouterCore, list: &mut [Candidate]) {
    list.sort_by_cached_key(|c| {
        !core
            .registered(&c.provider)
            .is_some_and(|r| permitted(&r, &c.model))
    });
}
