//! Polityka efektywna: sufit zawężony aktywnymi regułami — nigdy szersza niż sufit.

use std::collections::BTreeMap;

use safety_broker_contract::{AutonomyLevel, Capability};
use scheduler_contract::{OnTimeout, Priority};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::check::{Ceiling, check_rule};
use crate::rule::{Effect, PauseScope, Rule};

/// Polityka efektywna: sufit zawężony aktywnymi regułami.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EffectivePolicy {
    /// Poziom autonomii.
    pub autonomy: AutonomyLevel,
    /// Kroki.
    pub max_steps: u32,
    /// Czas (ms).
    pub max_wall_ms: u64,
    /// Koszt (mikro-PLN).
    pub max_cost_micro_pln: Option<u64>,
    /// Dozwolone zdolności (⊆ sufit).
    pub capabilities: Vec<Capability>,
    /// Odmówione zdolności.
    pub denied: Vec<Capability>,
    /// Odmówione rodziny.
    pub denied_families: Vec<String>,
    /// Rodziny wymagające zatwierdzenia.
    pub approval_families: Vec<String>,
    /// Zasoby: (najdłuższe czekanie, zachowanie po czasie) — najostrzejsze z reguł.
    pub exclusive: BTreeMap<String, (u64, OnTimeout)>,
    /// Priorytety wywłaszczane, gdy mówi użytkownik.
    pub preempt: Vec<Priority>,
    /// Zakresy wstrzymywane w punktach atomowych.
    pub pause: Vec<PauseScope>,
    /// Okna ciszy wyzwalaczy.
    pub quiet: Vec<(u16, u16)>,
    /// Mosty CLI zabronione.
    pub bridges_denied: bool,
    /// Zadania agentek naraz.
    pub max_parallel: u32,
}

fn push_unique<T: PartialEq>(v: &mut Vec<T>, x: T) {
    if !v.contains(&x) {
        v.push(x);
    }
}

/// Składa sufit z regułami. Reguły, które nie przechodzą [`check_rule`], są pomijane — wynik
/// nigdy nie jest szerszy niż sufit.
pub fn compose(ceiling: &Ceiling, rules: &[Rule]) -> EffectivePolicy {
    let mut p = EffectivePolicy {
        autonomy: ceiling.autonomy,
        max_steps: ceiling.max_steps,
        max_wall_ms: ceiling.max_wall_ms,
        max_cost_micro_pln: ceiling.max_cost_micro_pln,
        capabilities: ceiling.capabilities.clone(),
        denied: Vec::new(),
        denied_families: Vec::new(),
        approval_families: Vec::new(),
        exclusive: BTreeMap::new(),
        preempt: Vec::new(),
        pause: Vec::new(),
        quiet: Vec::new(),
        bridges_denied: false,
        max_parallel: ceiling.max_parallel,
    };
    for rule in rules.iter().filter(|r| check_rule(r, ceiling).is_empty()) {
        for effect in &rule.then {
            apply(&mut p, effect);
        }
    }
    let denied = p.denied.clone();
    let families = p.denied_families.clone();
    p.capabilities.retain(|c| {
        !denied.iter().any(|d| c.is_subset_of(d)) && !families.iter().any(|f| f == c.family())
    });
    p
}

fn apply(p: &mut EffectivePolicy, effect: &Effect) {
    match effect {
        Effect::Exclusive {
            resource,
            max_wait_ms,
            on_timeout,
        } => {
            let key = format!("{resource:?}");
            let entry = p
                .exclusive
                .entry(key)
                .or_insert((*max_wait_ms, *on_timeout));
            entry.0 = entry.0.min(*max_wait_ms);
            if *on_timeout == OnTimeout::AskUser {
                entry.1 = OnTimeout::AskUser;
            }
        }
        Effect::Preempt { classes } => classes.iter().for_each(|c| push_unique(&mut p.preempt, *c)),
        Effect::PauseAtAtomic { scope, .. } => push_unique(&mut p.pause, *scope),
        Effect::DenyCapability { capability } => push_unique(&mut p.denied, capability.clone()),
        Effect::DenyFamily { family } => push_unique(&mut p.denied_families, family.clone()),
        Effect::RestrictTo { capabilities } => {
            // Przecięcie w sensie „podzbioru”: zostaje węższa z każdej pary.
            let mut next = Vec::new();
            for c in &p.capabilities {
                for r in capabilities {
                    if c.is_subset_of(r) {
                        push_unique(&mut next, c.clone());
                    } else if r.is_subset_of(c) {
                        push_unique(&mut next, r.clone());
                    }
                }
            }
            p.capabilities = next;
        }
        Effect::RequireApproval { family } => push_unique(&mut p.approval_families, family.clone()),
        Effect::CapAutonomy { max } => p.autonomy = p.autonomy.min(*max),
        Effect::CapBudget {
            max_steps,
            max_wall_ms,
            max_cost_micro_pln,
        } => {
            if let Some(s) = max_steps {
                p.max_steps = p.max_steps.min(*s);
            }
            if let Some(w) = max_wall_ms {
                p.max_wall_ms = p.max_wall_ms.min(*w);
            }
            if let Some(c) = max_cost_micro_pln {
                p.max_cost_micro_pln = Some(p.max_cost_micro_pln.map_or(*c, |x| x.min(*c)));
            }
        }
        Effect::MaxParallel { n } => p.max_parallel = p.max_parallel.min(*n),
        Effect::QuietHours { start_min, end_min } => {
            push_unique(&mut p.quiet, (*start_min, *end_min))
        }
        Effect::DenyBridges => p.bridges_denied = true,
    }
}

/// Czy polityka mieści się w suficie (niezmiennik „reguły tylko zawężają”).
pub fn within(policy: &EffectivePolicy, ceiling: &Ceiling) -> bool {
    policy.autonomy <= ceiling.autonomy
        && policy.max_steps <= ceiling.max_steps
        && policy.max_wall_ms <= ceiling.max_wall_ms
        && match (policy.max_cost_micro_pln, ceiling.max_cost_micro_pln) {
            (_, None) => true,
            (Some(p), Some(c)) => p <= c,
            (None, Some(_)) => false,
        }
        && policy.max_parallel <= ceiling.max_parallel
        && policy
            .capabilities
            .iter()
            .all(|c| ceiling.capabilities.iter().any(|p| c.is_subset_of(p)))
}
