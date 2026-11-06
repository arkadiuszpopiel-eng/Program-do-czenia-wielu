//! Poziom autonomii przy zerwanym łączu z Sędzią (plan 50 §8.2). Dawniej zawsze L0, więc agentka
//! nagle przechodziła w tryb Plan, a pasek pokazywał „Plan”, choć właściciel niczego nie zmieniał.
//! Teraz: ostatni poziom odczytany z Sędziego dla tej sesji i agentki, ale nigdy wyżej niż L1
//! („Ręcznie” — pytanie o każdą zmianę); bez wcześniejszego odczytu — L0 jak dotąd. Decyzje i tak
//! zapadają w Sędzim, więc bez łącza każde działanie kończy się odmową.

use std::collections::BTreeMap;
use std::sync::Mutex;

use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::AutonomyLevel;

/// Najwyższy poziom podawany bez łącza.
const OFFLINE_CEILING: AutonomyLevel = AutonomyLevel::L1;
/// Najwięcej pamiętanych par (sesja, agentka) — potem pamięć zaczyna się od nowa.
const MAX_ENTRIES: usize = 256;

type Key = (SessionId, Option<AgentId>);

/// Ostatnie poziomy odczytane z Sędziego.
#[derive(Debug, Default)]
pub(crate) struct LastLevels(Mutex<BTreeMap<Key, AutonomyLevel>>);

impl LastLevels {
    fn map(&self) -> std::sync::MutexGuard<'_, BTreeMap<Key, AutonomyLevel>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Zapamiętuje poziom odczytany z Sędziego i zwraca go bez zmian.
    pub(crate) fn remember(
        &self,
        session: &SessionId,
        agent: Option<&AgentId>,
        level: AutonomyLevel,
    ) -> AutonomyLevel {
        let mut map = self.map();
        if map.len() >= MAX_ENTRIES {
            map.clear();
        }
        map.insert((session.clone(), agent.cloned()), level);
        level
    }

    /// Poziom bez łącza: ostatni znany, najwyżej L1; bez odczytu — L0.
    pub(crate) fn offline(&self, session: &SessionId, agent: Option<&AgentId>) -> AutonomyLevel {
        self.map()
            .get(&(session.clone(), agent.cloned()))
            .map_or(AutonomyLevel::L0, |l| (*l).min(OFFLINE_CEILING))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_level_keeps_plan_and_never_exceeds_manual() {
        let last = LastLevels::default();
        let (s, a) = (SessionId::new("s"), AgentId::new("delta"));
        assert_eq!(last.offline(&s, None), AutonomyLevel::L0, "bez odczytu");
        last.remember(&s, None, AutonomyLevel::L4);
        assert_eq!(last.offline(&s, None), AutonomyLevel::L1);
        assert_eq!(
            last.offline(&s, Some(&a)),
            AutonomyLevel::L0,
            "inna agentka"
        );
        last.remember(&s, Some(&a), AutonomyLevel::L0);
        assert_eq!(
            last.offline(&s, Some(&a)),
            AutonomyLevel::L0,
            "Plan zostaje"
        );
        for i in 0..MAX_ENTRIES {
            last.remember(&SessionId::new(format!("x{i}")), None, AutonomyLevel::L3);
        }
        assert!(last.map().len() <= MAX_ENTRIES);
    }
}
