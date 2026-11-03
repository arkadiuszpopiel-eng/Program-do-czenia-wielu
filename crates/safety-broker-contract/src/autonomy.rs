//! Poziomy autonomii per sesja/agentka (PLAN §8.3, ADR 15). Zmienia je wyłącznie Broker;
//! podniesienie tylko przez `ApprovalChannel` z dowodem fizycznego wejścia.

use std::collections::BTreeMap;

use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::AutonomyLevel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Gdzie działa model przebiegu — z kontekstu wywołania (dostawca wybrany przez runtime), nigdy
/// od modelu. Brak informacji Sędzia traktuje jak chmurę (plan 50 §8.1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ModelLocation {
    /// Model na tym komputerze (llama.cpp, Ollama, LM Studio — dostawca `local`).
    Local,
    /// Model u dostawcy w Internecie (klucz API, most CLI).
    Cloud,
}

impl ModelLocation {
    /// Pochodzenie z identyfikatora modelu `dostawca:model` (Router) albo nazwy dostawcy:
    /// lokalny tylko dostawca `local`; wszystko inne — chmura.
    pub fn of_model(model: &str) -> Self {
        let provider = model.split_once(':').map_or(model, |(p, _)| p);
        if provider.trim().eq_ignore_ascii_case("local") {
            Self::Local
        } else {
            Self::Cloud
        }
    }
}

/// Zasięg przełącznika „Max dostęp” (decyzja właściciela 3.11, plan 50 §7): żadnych pytań dla
/// przebiegów na modelach lokalnych, w chmurze albo na wszystkich.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ModelScope {
    /// Modele lokalne.
    Local,
    /// Modele w chmurze (także przebiegi bez informacji o modelu).
    Cloud,
    /// Wszystkie modele.
    All,
}

impl ModelScope {
    /// Czy zasięg obejmuje przebieg o danym pochodzeniu (`None` = nieznane → jak chmura).
    pub fn covers(self, location: Option<ModelLocation>) -> bool {
        match self {
            Self::All => true,
            Self::Local => location == Some(ModelLocation::Local),
            Self::Cloud => location != Some(ModelLocation::Local),
        }
    }
}

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
    /// „Max dostęp” — przebiegi na modelach z zasięgu (najwyżej jeden wpis naraz; ponad
    /// wszystkimi innymi wpisami, bo właściciel włączył go świadomie).
    Models {
        /// Zasięg.
        scope: ModelScope,
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
    /// Ustawia wpis. „Max dostęp” jest jednym przełącznikiem: nowy wpis `Models` zastępuje
    /// poprzedni, a poziom nie wyższy niż domyślny go wyłącza.
    pub fn set(&mut self, target: AutonomyTarget, entry: AutonomyEntry) {
        if matches!(target, AutonomyTarget::Models { .. }) {
            self.entries
                .retain(|t, _| !matches!(t, AutonomyTarget::Models { .. }));
            if entry.level <= AutonomyLevel::default() {
                return;
            }
        }
        self.entries.insert(target, entry);
    }

    /// Włączony „Max dostęp” (zasięg i poziom), gdy aktywny.
    pub fn max_access(&self, now_ms: u64) -> Option<(ModelScope, AutonomyLevel)> {
        self.entries.iter().find_map(|(t, e)| match t {
            AutonomyTarget::Models { scope } if e.until_ms.is_none_or(|u| now_ms < u) => {
                Some((*scope, e.level))
            }
            _ => None,
        })
    }

    /// Poziom obowiązujący przebieg: jak [`AutonomyTable::effective`], a gdy „Max dostęp”
    /// obejmuje pochodzenie modelu — nie niższy niż jego poziom.
    pub fn effective_for(
        &self,
        session: &SessionId,
        agent: Option<&AgentId>,
        location: Option<ModelLocation>,
        now_ms: u64,
    ) -> AutonomyLevel {
        let base = self.effective(session, agent, now_ms);
        match self.max_access(now_ms) {
            Some((scope, level)) if scope.covers(location) => base.max(level),
            _ => base,
        }
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

    /// Podmiot bez własnego poziomu (sesji, agentki, agentki w sesji) — obowiązuje go tryb
    /// globalny.
    pub fn follows_global(
        &self,
        session: &SessionId,
        agent: Option<&AgentId>,
        now_ms: u64,
    ) -> bool {
        let own = |target: AutonomyTarget| self.get(&target, now_ms).is_some();
        let session_own = own(AutonomyTarget::Session {
            session: session.clone(),
        });
        let agent_own = agent.is_some_and(|a| {
            own(AutonomyTarget::Agent { agent: a.clone() })
                || own(AutonomyTarget::SessionAgent {
                    session: session.clone(),
                    agent: a.clone(),
                })
        });
        !session_own && !agent_own
    }

    /// Wpis globalny bez terminu (zmiana właściciela).
    pub fn standing_global(&self) -> Option<AutonomyLevel> {
        self.entries
            .get(&AutonomyTarget::Global)
            .filter(|e| e.until_ms.is_none())
            .map(|e| e.level)
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

    #[test]
    fn max_access_covers_model_locations() {
        let s = SessionId::new("s1");
        let a = AgentId::new("delta");
        let local = Some(ModelLocation::Local);
        let cloud = Some(ModelLocation::Cloud);
        let mut t = AutonomyTable::default();
        t.set(
            AutonomyTarget::Session { session: s.clone() },
            entry(AutonomyLevel::L1),
        );
        assert_eq!(t.effective_for(&s, Some(&a), local, 0), AutonomyLevel::L1);
        let models = |scope| AutonomyTarget::Models { scope };
        t.set(models(ModelScope::Local), entry(AutonomyLevel::L4));
        assert_eq!(t.effective_for(&s, Some(&a), local, 0), AutonomyLevel::L4);
        assert_eq!(t.effective_for(&s, Some(&a), cloud, 0), AutonomyLevel::L1);
        assert_eq!(t.effective_for(&s, Some(&a), None, 0), AutonomyLevel::L1);
        // Jeden przełącznik: „chmura” zastępuje „lokalne”.
        t.set(models(ModelScope::Cloud), entry(AutonomyLevel::L4));
        assert_eq!(
            t.max_access(0),
            Some((ModelScope::Cloud, AutonomyLevel::L4))
        );
        assert_eq!(t.effective_for(&s, Some(&a), local, 0), AutonomyLevel::L1);
        assert_eq!(t.effective_for(&s, Some(&a), None, 0), AutonomyLevel::L4);
        t.set(models(ModelScope::All), entry(AutonomyLevel::L4));
        assert_eq!(t.effective_for(&s, Some(&a), local, 0), AutonomyLevel::L4);
        assert_eq!(t.entries().len(), 2);
        // Wyłączenie: poziom domyślny albo niższy usuwa wpis.
        t.set(models(ModelScope::All), entry(AutonomyLevel::L3));
        assert_eq!(t.max_access(0), None);
        assert_eq!(t.effective(&s, Some(&a), 0), AutonomyLevel::L1);
        assert_eq!(t.effective_for(&s, Some(&a), local, 0), AutonomyLevel::L1);
    }

    #[test]
    fn model_location_from_router_ids() {
        assert_eq!(
            ModelLocation::of_model("local:bielik-4.5b"),
            ModelLocation::Local
        );
        assert_eq!(ModelLocation::of_model("LOCAL"), ModelLocation::Local);
        assert_eq!(
            ModelLocation::of_model("anthropic:claude"),
            ModelLocation::Cloud
        );
        assert_eq!(ModelLocation::of_model("auto"), ModelLocation::Cloud);
        assert_eq!(ModelLocation::of_model("localhost:x"), ModelLocation::Cloud);
    }
}
