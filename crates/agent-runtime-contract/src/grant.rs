//! Koperta uprawnień przebiegu (v1): narzędzia, rodziny zdolności, tylko odczyt, sufit budżetu
//! i autonomii. Przy delegacji i uruchamianiu umiejętności koperta potomka powstaje przez
//! [`RunGrant::attenuate`] — **potomek ≤ rodzic** w każdym wymiarze (PLAN §8.1). Tokeny Brokera
//! i tak są wydawane per akcja; koperta zawęża to, o co przebieg w ogóle może poprosić.

use std::collections::BTreeSet;

use risk_classifier_contract::AutonomyLevel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::RunBudget;

/// Koperta uprawnień przebiegu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunGrant {
    /// Narzędzia dozwolone (nazwy dla modelu).
    pub tools: BTreeSet<String>,
    /// Rodziny zdolności (`fs.read`, `fs.write`, `shell.exec`, `gui.control`, `net.egress`…).
    pub capabilities: BTreeSet<String>,
    /// Tylko odczyt — narzędzia zmieniające stan niedostępne.
    #[serde(default)]
    pub read_only: bool,
    /// Sufit budżetu.
    pub budget: RunBudget,
    /// Sufit poziomu autonomii (`None` = bez sufitu runtime; decyduje Broker).
    #[serde(default)]
    pub max_autonomy: Option<AutonomyLevel>,
}

fn min_opt<T: Ord + Copy>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

/// Mniejszy z budżetów (każde pole osobno; brak limitu kosztu przegrywa z limitem).
pub fn min_budget(a: &RunBudget, b: &RunBudget) -> RunBudget {
    RunBudget {
        max_steps: a.max_steps.min(b.max_steps),
        max_tokens: a.max_tokens.min(b.max_tokens),
        max_wall_ms: a.max_wall_ms.min(b.max_wall_ms),
        max_cost_micro_usd: min_opt(a.max_cost_micro_usd, b.max_cost_micro_usd),
        max_tool_calls_per_turn: a.max_tool_calls_per_turn.min(b.max_tool_calls_per_turn),
    }
}

/// Czy budżet `child` mieści się w `parent`.
pub fn budget_within(child: &RunBudget, parent: &RunBudget) -> bool {
    let cost_ok = match (child.max_cost_micro_usd, parent.max_cost_micro_usd) {
        (_, None) => true,
        (Some(c), Some(p)) => c <= p,
        (None, Some(_)) => false,
    };
    child.max_steps <= parent.max_steps
        && child.max_tokens <= parent.max_tokens
        && child.max_wall_ms <= parent.max_wall_ms
        && child.max_tool_calls_per_turn <= parent.max_tool_calls_per_turn
        && cost_ok
}

impl RunGrant {
    /// Koperta bez ograniczeń narzędzi (pełny zestaw nazw i rodzin podany przez wywołującego).
    pub fn new(
        tools: impl IntoIterator<Item = String>,
        capabilities: impl IntoIterator<Item = String>,
        budget: RunBudget,
    ) -> Self {
        Self {
            tools: tools.into_iter().collect(),
            capabilities: capabilities.into_iter().collect(),
            read_only: false,
            budget,
            max_autonomy: None,
        }
    }

    /// Atenuacja: przecięcie narzędzi i zdolności, „tylko odczyt” dziedziczony, mniejszy budżet
    /// i niższy sufit autonomii. Wynik zawsze mieści się w `self` ([`RunGrant::is_within`]).
    pub fn attenuate(&self, request: &RunGrant) -> RunGrant {
        RunGrant {
            tools: self.tools.intersection(&request.tools).cloned().collect(),
            capabilities: self
                .capabilities
                .intersection(&request.capabilities)
                .cloned()
                .collect(),
            read_only: self.read_only || request.read_only,
            budget: min_budget(&self.budget, &request.budget),
            max_autonomy: min_opt(self.max_autonomy, request.max_autonomy),
        }
    }

    /// Czy koperta mieści się w kopercie rodzica (potomek ≤ rodzic).
    pub fn is_within(&self, parent: &RunGrant) -> bool {
        let autonomy_ok = match (self.max_autonomy, parent.max_autonomy) {
            (_, None) => true,
            (Some(c), Some(p)) => c <= p,
            (None, Some(_)) => false,
        };
        self.tools.is_subset(&parent.tools)
            && self.capabilities.is_subset(&parent.capabilities)
            && (self.read_only || !parent.read_only)
            && budget_within(&self.budget, &parent.budget)
            && autonomy_ok
    }

    /// Czy narzędzie (nazwa, rodziny zdolności z manifestu, czy zmienia stan) mieści się w kopercie.
    pub fn permits(&self, tool: &str, capabilities: &[String], mutating: bool) -> bool {
        self.tools.contains(tool)
            && capabilities.iter().all(|c| self.capabilities.contains(c))
            && !(self.read_only && mutating)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn names() -> impl Strategy<Value = BTreeSet<String>> {
        proptest::collection::btree_set("[a-e]{1,2}", 0..6)
    }

    fn budget() -> impl Strategy<Value = RunBudget> {
        (
            1u32..100,
            1u64..1000,
            1u64..10_000,
            proptest::option::of(0u64..500),
            1u32..10,
        )
            .prop_map(|(s, t, w, c, p)| RunBudget {
                max_steps: s,
                max_tokens: t,
                max_wall_ms: w,
                max_cost_micro_usd: c,
                max_tool_calls_per_turn: p,
            })
    }

    fn level() -> impl Strategy<Value = Option<AutonomyLevel>> {
        proptest::option::of(prop_oneof![
            Just(AutonomyLevel::L0),
            Just(AutonomyLevel::L1),
            Just(AutonomyLevel::L2),
            Just(AutonomyLevel::L3),
            Just(AutonomyLevel::L4),
        ])
    }

    fn grant() -> impl Strategy<Value = RunGrant> {
        (names(), names(), any::<bool>(), budget(), level()).prop_map(|(t, c, r, b, a)| RunGrant {
            tools: t,
            capabilities: c,
            read_only: r,
            budget: b,
            max_autonomy: a,
        })
    }

    proptest! {
        /// Atenuacja nigdy nie rozszerza uprawnień i jest idempotentna.
        #[test]
        fn attenuation_never_widens(parent in grant(), request in grant()) {
            let child = parent.attenuate(&request);
            prop_assert!(child.is_within(&parent));
            prop_assert!(child.is_within(&request));
            prop_assert_eq!(child.attenuate(&request), child.clone());
            for t in &child.tools {
                prop_assert!(parent.tools.contains(t));
            }
            prop_assert!(parent.attenuate(&parent).is_within(&parent));
        }
    }

    #[test]
    fn permits_respects_read_only_and_families() {
        let mut g = RunGrant::new(
            ["fs_read".to_owned(), "fs_write".to_owned()],
            ["fs.read".to_owned(), "fs.write".to_owned()],
            RunBudget::default(),
        );
        assert!(g.permits("fs_write", &["fs.write".into()], true));
        assert!(!g.permits("shell_run", &["shell.exec".into()], true));
        assert!(!g.permits("fs_read", &["net.egress".into()], false));
        g.read_only = true;
        assert!(!g.permits("fs_write", &["fs.write".into()], true));
        assert!(g.permits("fs_read", &["fs.read".into()], false));
        let mut wide = g.clone();
        wide.read_only = false;
        assert!(
            !wide.is_within(&g),
            "zdjęcie „tylko odczyt” rozszerza uprawnienia"
        );
        let mut costly = g.clone();
        costly.budget.max_cost_micro_usd = Some(5);
        assert!(costly.is_within(&g) && !g.is_within(&costly));
        let mut l4 = g.clone();
        l4.max_autonomy = None;
        let mut l2 = g.clone();
        l2.max_autonomy = Some(AutonomyLevel::L2);
        assert!(l2.is_within(&l4) && !l4.is_within(&l2));
    }
}
