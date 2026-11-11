//! Lokalny8–12B do pracy głównej, skonfigurowana lekka chmura do pomocy.
use crate::routing::{Registered, RouterCore};
use providers_contract::{HealthState, ModelKind};
use router_contract::{Candidate, DifficultyNote, RouteKind, Rung, TaskClass, params_billions};

fn helper(core: &RouterCore, candidate: &Candidate) -> bool {
    let rung = core.rung_of(candidate);
    rung == Rung::Light
        || (rung != Rung::Strong
            && candidate
                .model
                .to_ascii_lowercase()
                .split(['-', '_', '/', ':'])
                .any(|p| p == "luna"))
}

pub(crate) fn api_candidates(
    core: &RouterCore,
    reg: &[(providers_contract::ProviderId, Registered)],
) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (id, r) in reg {
        if r.kind != RouteKind::Api || r.provider.health().state == HealthState::Unconfigured {
            continue;
        }
        let caps = r.provider.capabilities();
        if let Some(m) = caps.default_model {
            out.push(Candidate::new(id.clone(), m));
        }
        for (m, c) in caps.models {
            let candidate = Candidate::new(id.clone(), &m);
            if helper(core, &candidate) && c.supports(ModelKind::Chat) && !out.contains(&candidate)
            {
                out.push(candidate);
            }
        }
    }
    out.sort_by_cached_key(|c| u8::from(!helper(core, c)));
    out
}

fn main_local(core: &RouterCore, c: &Candidate) -> bool {
    core.registered(&c.provider)
        .is_some_and(|r| r.kind == RouteKind::Local)
        && core.light_id().as_ref() != Some(&c.provider)
        && params_billions(&c.model).is_some_and(|p| (8.0..=12.0).contains(&p))
}

pub(crate) fn prefer(
    core: &RouterCore,
    class: TaskClass,
    mut list: Vec<Candidate>,
    note: &DifficultyNote,
    review: bool,
) -> Vec<Candidate> {
    if core.manual_queue(class)
        || note.start == Rung::Strong
        || note.escalated_from.is_some()
        || !core.difficulty_policy().local_first
        || !(list.iter().any(|c| main_local(core, c))
            || core.all_registered().iter().any(|(id, r)| {
                r.kind == RouteKind::Local
                    && core.light_id().as_ref() != Some(id)
                    && r.provider
                        .capabilities()
                        .models
                        .keys()
                        .any(|m| params_billions(m).is_some_and(|p| (8.0..=12.0).contains(&p)))
            }))
    {
        return list;
    }
    // Ranking nie omija check(): prywatność, możliwości, stan i budżet pozostają bramkami.
    list.sort_by_cached_key(|c| {
        if main_local(core, c) {
            return u8::from(review);
        }
        if core
            .registered(&c.provider)
            .is_some_and(|r| r.kind == RouteKind::Api)
        {
            if helper(core, c) {
                u8::from(!review)
            } else if core.rung_of(c) == Rung::Strong {
                3
            } else {
                2
            }
        } else {
            4
        }
    });
    list
}
