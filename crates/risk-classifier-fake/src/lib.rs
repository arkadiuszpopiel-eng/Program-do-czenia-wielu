//! Atrapa klasyfikatora ryzyka (docs/modules/risk-classifier/SPEC.md, sekcja „Fake”).
//!
//! Domyślnie liczy werdykt tą samą tabelą co `-impl` (`evaluate` z kontraktu). Testy innych
//! modułów mogą zaskryptować werdykt per narzędzie; **twardych blokad Jądra nie da się
//! zaskryptować** — fakty z `kernel_rule` zawsze dają `HardBlock`, żeby atrapa nie uczyła
//! innych modułów błędnych założeń.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use risk_classifier_contract::{
    ActionFacts, AutonomyLevel, RiskClassifier, RiskLevel, RiskPolicy, RiskVerdict, Verdict,
    evaluate,
};

#[derive(Debug, Default)]
struct State {
    scripted: BTreeMap<String, (RiskLevel, Verdict)>,
    calls: Vec<(ActionFacts, AutonomyLevel)>,
}

/// Deterministyczna atrapa klasyfikatora.
#[derive(Debug, Default)]
pub struct FakeClassifier {
    policy: RiskPolicy,
    state: Mutex<State>,
}

impl FakeClassifier {
    /// Atrapa z domyślnymi progami.
    pub fn new() -> Self {
        Self::default()
    }

    /// Atrapa z własnymi progami.
    pub fn with_policy(policy: RiskPolicy) -> Self {
        Self {
            policy,
            state: Mutex::default(),
        }
    }

    /// Skryptuje werdykt dla narzędzia (nie działa dla faktów z regułą Jądra).
    pub fn script(&self, tool: &str, level: RiskLevel, verdict: Verdict) {
        self.lock()
            .scripted
            .insert(tool.to_owned(), (level, verdict));
    }

    /// Usuwa wszystkie skrypty.
    pub fn clear_script(&self) {
        self.lock().scripted.clear();
    }

    /// Wszystkie wywołania `evaluate` (fakty, poziom).
    pub fn calls(&self) -> Vec<(ActionFacts, AutonomyLevel)> {
        self.lock().calls.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl RiskClassifier for FakeClassifier {
    fn policy(&self) -> RiskPolicy {
        self.policy
    }

    fn evaluate(&self, facts: &ActionFacts, autonomy: AutonomyLevel) -> RiskVerdict {
        let mut st = self.lock();
        st.calls.push((facts.clone(), autonomy));
        let table = evaluate(facts, autonomy, &self.policy);
        if facts.kernel_rule.is_some() {
            return table;
        }
        match st.scripted.get(&facts.tool) {
            Some((level, verdict)) => RiskVerdict {
                level: *level,
                verdict: *verdict,
                rules: Vec::new(),
                factors: vec!["werdykt zaskryptowany w atrapie".to_owned()],
                explanation: format!("Atrapa: werdykt zaskryptowany dla `{}`.", facts.tool),
            },
            None => table,
        }
    }
}
