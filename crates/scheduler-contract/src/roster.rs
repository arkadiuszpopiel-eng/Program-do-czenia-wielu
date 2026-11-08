//! Obsada dostępna dla schedulera (agentki, role, pojemność) i warunki systemowe (bezczynność,
//! tryb gry). Przydział jest deterministyczny: najmniej zajęta, potem kolejność w obsadzie.

use std::collections::BTreeMap;

use personas_contract::{Cast, PersonaId, RoleId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::Assignee;
use crate::state::BlockReason;

/// Agentka w obsadzie schedulera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentSlot {
    /// Persona.
    pub persona: PersonaId,
    /// Role w bieżącej obsadzie.
    pub roles: Vec<RoleId>,
    /// Dostępna (np. wyłączona w Ustawieniach = `false`).
    pub available: bool,
    /// Ile zadań naraz (≥ 1).
    pub max_parallel: u32,
}

/// Obsada i limity równoległości.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Roster {
    /// Agentki (kolejność = pierwszeństwo przy remisie).
    pub agents: Vec<AgentSlot>,
    /// Najwięcej zadań agentek naraz.
    pub max_parallel_total: u32,
    /// Najwięcej zadań usług systemowych naraz.
    pub max_parallel_system: u32,
}

impl Default for Roster {
    /// Obsada „Standard” (PLAN §9.2), każda agentka jedno zadanie naraz, razem 4.
    fn default() -> Self {
        let slot = |p: PersonaId, roles: Vec<RoleId>| AgentSlot {
            persona: p,
            roles,
            available: true,
            max_parallel: 1,
        };
        Self {
            agents: vec![
                slot(
                    PersonaId::alfa(),
                    vec![RoleId::conductor(), RoleId::speaker()],
                ),
                slot(PersonaId::beta(), vec![RoleId::keeper(), RoleId::writer()]),
                slot(
                    PersonaId::gama(),
                    vec![RoleId::researcher(), RoleId::critic(), RoleId::thinker()],
                ),
                slot(
                    PersonaId::new("delta"),
                    vec![RoleId::operator(), RoleId::coder()],
                ),
            ],
            max_parallel_total: 4,
            max_parallel_system: 2,
        }
    }
}

impl Roster {
    /// Obsada z `personas` (role z bieżącej obsady, każda agentka jedno zadanie naraz).
    pub fn from_cast(cast: &Cast, max_parallel_total: u32) -> Self {
        Self {
            agents: cast
                .assignments
                .iter()
                .map(|(persona, roles)| AgentSlot {
                    persona: persona.clone(),
                    roles: roles.iter().cloned().collect(),
                    available: true,
                    max_parallel: 1,
                })
                .collect(),
            max_parallel_total,
            max_parallel_system: 2,
        }
    }

    /// Wybiera agentkę dla przydziału. `Ok(None)` = usługa systemowa (bez agentki).
    /// Błąd = powód czekania (`NoAgent` — nikt w obsadzie się nie nadaje; `AgentBusy` — zajęte).
    pub fn pick(
        &self,
        assignee: &Assignee,
        running: &BTreeMap<PersonaId, u32>,
    ) -> Result<Option<PersonaId>, BlockReason> {
        let eligible: Vec<&AgentSlot> = match assignee {
            Assignee::System(_) => return Ok(None),
            Assignee::Persona(p) => self.agents.iter().filter(|a| &a.persona == p).collect(),
            Assignee::Role(r) => self.agents.iter().filter(|a| a.roles.contains(r)).collect(),
            Assignee::AnyAgent => self.agents.iter().collect(),
        };
        let available: Vec<&AgentSlot> = eligible.into_iter().filter(|a| a.available).collect();
        if available.is_empty() {
            return Err(BlockReason::NoAgent);
        }
        let load = |a: &AgentSlot| running.get(&a.persona).copied().unwrap_or(0);
        available
            .iter()
            .enumerate()
            .filter(|(_, a)| load(a) < a.max_parallel.max(1))
            .min_by_key(|(i, a)| (load(a), *i))
            .map(|(_, a)| Some(a.persona.clone()))
            .ok_or(BlockReason::AgentBusy)
    }
}

/// Warunki systemowe dla okien czasowych (dostarcza powłoka: bezczynność z platformy, tryb gry
/// z `model-residency`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SystemConditions {
    /// Użytkownik bezczynny.
    pub user_idle: bool,
    /// Tryb gry / pełny ekran.
    pub game_mode: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_least_loaded_then_order() {
        let roster = Roster::default();
        let mut running = BTreeMap::new();
        assert_eq!(
            roster.pick(&Assignee::AnyAgent, &running),
            Ok(Some(PersonaId::alfa()))
        );
        running.insert(PersonaId::alfa(), 1);
        assert_eq!(
            roster.pick(&Assignee::AnyAgent, &running),
            Ok(Some(PersonaId::beta()))
        );
        assert_eq!(
            roster.pick(&Assignee::Role(RoleId::conductor()), &running),
            Err(BlockReason::AgentBusy)
        );
        assert_eq!(
            roster.pick(&Assignee::Role(RoleId::new("nieznana")), &running),
            Err(BlockReason::NoAgent)
        );
        assert_eq!(
            roster.pick(&Assignee::System("x".into()), &running),
            Ok(None)
        );
        let mut off = roster.clone();
        off.agents[1].available = false;
        assert_eq!(
            off.pick(&Assignee::Persona(PersonaId::beta()), &running),
            Err(BlockReason::NoAgent)
        );
        let cast = Cast::solo(PersonaId::gama(), [RoleId::critic()], false);
        let solo = Roster::from_cast(&cast, 2);
        assert_eq!(solo.agents.len(), 1);
        assert_eq!(
            solo.pick(&Assignee::Role(RoleId::critic()), &BTreeMap::new()),
            Ok(Some(PersonaId::gama()))
        );
    }
}
