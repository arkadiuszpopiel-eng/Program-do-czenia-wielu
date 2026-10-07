//! Części atrapy: klasyfikator ze skryptem i port procesów bez zabijania.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard};

use platform_contract::{PlatformError, ProcessHandle, ProcessPort, ProcessSpec, ProcessStatus};
use risk_classifier_contract::{
    ActionFacts, AutonomyLevel, KernelRule, RiskClassifier, RiskPolicy, RiskVerdict, Verdict,
    evaluate,
};

/// Skryptowana decyzja dla narzędzia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptedDecision {
    /// Wydaj token bez pytania (o ile nie narusza reguł Jądra).
    Allow,
    /// Zawsze pytaj.
    NeedsApproval,
    /// Odmów z podaną regułą.
    Deny(KernelRule),
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Klasyfikator: tabela z kontraktu + skrypt per narzędzie; reguły Jądra nie do zdjęcia.
#[derive(Debug)]
pub struct ScriptedClassifier {
    policy: RiskPolicy,
    script: Mutex<BTreeMap<String, ScriptedDecision>>,
}

impl ScriptedClassifier {
    /// Klasyfikator z progami.
    pub fn new(policy: RiskPolicy) -> Self {
        Self {
            policy,
            script: Mutex::new(BTreeMap::new()),
        }
    }

    /// Ustawia skrypt narzędzia.
    pub fn set(&self, tool: &str, d: ScriptedDecision) {
        lock(&self.script).insert(tool.to_owned(), d);
    }

    /// Skrypt narzędzia.
    pub fn get(&self, tool: &str) -> Option<ScriptedDecision> {
        lock(&self.script).get(tool).copied()
    }
}

impl RiskClassifier for ScriptedClassifier {
    fn policy(&self) -> RiskPolicy {
        self.policy
    }

    fn evaluate(&self, facts: &ActionFacts, autonomy: AutonomyLevel) -> RiskVerdict {
        let mut v = evaluate(facts, autonomy, &self.policy);
        if facts.kernel_rule.is_some() {
            return v;
        }
        match self.get(&facts.tool) {
            Some(ScriptedDecision::Allow) => v.verdict = Verdict::Proceed,
            Some(ScriptedDecision::NeedsApproval) => {
                v.verdict = Verdict::Ask {
                    non_voice: false,
                    grantable: false,
                }
            }
            Some(ScriptedDecision::Deny(rule)) => v.verdict = Verdict::HardBlock { rule },
            None => {}
        }
        v
    }
}

/// Port procesów atrapy: każde drzewo „zabija” tylko w rejestrze.
#[derive(Debug, Default)]
pub struct FakeProcesses {
    killed: Mutex<BTreeSet<u32>>,
}

impl FakeProcesses {
    /// „Zabite” uchwyty rosnąco.
    pub fn killed(&self) -> Vec<u32> {
        lock(&self.killed).iter().copied().collect()
    }
}

impl ProcessPort for FakeProcesses {
    fn spawn(&self, _: ProcessSpec) -> Result<ProcessHandle, PlatformError> {
        Err(PlatformError::Unsupported(
            "atrapa Brokera nie uruchamia procesów".into(),
        ))
    }

    fn kill_tree(&self, handle: ProcessHandle) -> Result<(), PlatformError> {
        lock(&self.killed).insert(handle.0);
        Ok(())
    }

    fn status(&self, handle: ProcessHandle) -> Result<ProcessStatus, PlatformError> {
        if lock(&self.killed).contains(&handle.0) {
            Ok(ProcessStatus::Killed)
        } else {
            Ok(ProcessStatus::Running)
        }
    }

    fn foreground_is_elevated(&self) -> bool {
        false
    }
}
