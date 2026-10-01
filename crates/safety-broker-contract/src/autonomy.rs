//! Poziomy autonomii per sesja/agentka (PLAN §8.3, ADR 15). Zmienia je wyłącznie Broker;
//! podniesienie tylko przez `ApprovalChannel` z dowodem fizycznego wejścia.

use std::collections::BTreeMap;

use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::AutonomyLevel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Cel ustawienia poziomu.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum AutonomyTarget {
    /// Cała aplikacja.
    Global,
    /// Jedna sesja (wszystkie agentki).
    Session {
        /// Sesja.
        session: SessionId,
    },
    /// Jedna agentka (we wszystkich sesjach).
    Agent {
        /// Agentka.
        agent: AgentId,
    },
    /// Agentka w jednej sesji (najbardziej szczegółowe).
    SessionAgent {
        /// Sesja.
        session: SessionId,
        /// Agentka.
        agent: AgentId,
    },
}

/// Wpis: poziom z opcjonalnym terminem („na czas”).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AutonomyEntry {
    /// Poziom.
    pub level: AutonomyLevel,
    /// Do kiedy obowiązuje (ms); `None` = bezterminowo.
    pub until_ms: Option<u64>,
}

/// Tabela poziomów. Rozstrzyganie: `SessionAgent` → min(`Session`, `Agent`) → `Global` → L3.
/// Minimum przy konflikcie sesji i agentki = sufit sesji (bezpieczniejsza interpretacja).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyTable {
    entries: BTreeMap<AutonomyTarget, AutonomyEntry>,
}

impl AutonomyTable {
    /// Ustawia wpis.
    pub fn set(&mut self, target: AutonomyTarget, entry: AutonomyEntry) {
        self.entries.insert(target, entry);
    }

    /// Usuwa wpis (powrót do reguły ogólniejszej).
    pub fn clear(&mut self, target: &AutonomyTarget) -> bool {
        self.entries.remove(target).is_some()
    }

    /// Aktywny wpis celu (po terminie — brak).
    pub fn get(&self, target: &AutonomyTarget, now_ms: u64) -> Option<AutonomyLevel> {
        self.entries
            .get(target)
            .filter(|e| e.until_ms.is_none_or(|until| now_ms < until))
            .map(|e| e.level)
    }

    /// Poziom obowiązujący agentkę w sesji.
    pub fn effective(
        &self,
        session: &SessionId,
        agent: Option<&AgentId>,
        now_ms: u64,
    ) -> AutonomyLevel {
        if let Some(agent) = agent {
            let pair = AutonomyTarget::SessionAgent {
                session: session.clone(),
                agent: agent.clone(),
            };
            if let Some(level) = self.get(&pair, now_ms) {
                return level;
            }
        }
        let by_session = self.get(
            &AutonomyTarget::Session {
                session: session.clone(),
            },
            now_ms,
        );
        let by_agent =
            agent.and_then(|a| self.get(&AutonomyTarget::Agent { agent: a.clone() }, now_ms));
        match (by_session, by_agent) {
            (Some(s), Some(a)) => s.min(a),
            (Some(l), None) | (None, Some(l)) => l,
            (None, None) => self
                .get(&AutonomyTarget::Global, now_ms)
                .unwrap_or_default(),
        }
    }

    /// Wszystkie wpisy (do UI ustawień).
    pub fn entries(&self) -> Vec<(AutonomyTarget, AutonomyEntry)> {
        self.entries.iter().map(|(t, e)| (t.clone(), *e)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(level: AutonomyLevel) -> AutonomyEntry {
        AutonomyEntry {
            level,
            until_ms: None,
        }
    }

    #[test]
    fn resolution_order_and_expiry() {
        let s = SessionId::new("s1");
        let a = AgentId::new("delta");
        let mut t = AutonomyTable::default();
        assert_eq!(t.effective(&s, Some(&a), 0), AutonomyLevel::L3);
        t.set(AutonomyTarget::Global, entry(AutonomyLevel::L4));
        assert_eq!(t.effective(&s, Some(&a), 0), AutonomyLevel::L4);
        t.set(
            AutonomyTarget::Session { session: s.clone() },
            entry(AutonomyLevel::L2),
        );
        t.set(
            AutonomyTarget::Agent { agent: a.clone() },
            entry(AutonomyLevel::L4),
        );
        assert_eq!(t.effective(&s, Some(&a), 0), AutonomyLevel::L2);
        t.set(
            AutonomyTarget::SessionAgent {
                session: s.clone(),
                agent: a.clone(),
            },
            AutonomyEntry {
                level: AutonomyLevel::L4,
                until_ms: Some(100),
            },
        );
        assert_eq!(t.effective(&s, Some(&a), 99), AutonomyLevel::L4);
        assert_eq!(t.effective(&s, Some(&a), 100), AutonomyLevel::L2);
        assert_eq!(t.effective(&s, None, 0), AutonomyLevel::L2);
        assert!(t.clear(&AutonomyTarget::Session { session: s.clone() }));
        assert_eq!(t.effective(&s, Some(&a), 200), AutonomyLevel::L4);
        assert_eq!(t.entries().len(), 3);
    }
}
