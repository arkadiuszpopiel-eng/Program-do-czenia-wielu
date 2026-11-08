//! Kto pyta o pamięć i do jakich zakresów ma dostęp (PLAN §10: „dzielenie tylko jawnie”;
//! §9.5: `memory: { scope, retain }` w manifeście roli; §8.5: mosty CLI tylko `recall`).

use core_bus_contract::{AgentId, SessionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::MemoryError;
use crate::types::MemoryScope;

/// Zakres względny przyznany agentce (wobec jej sesji, projektu sesji i jej samej).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScopeGrant {
    /// Własna sesja (zawsze domyślny).
    Session,
    /// Projekt, do którego należy sesja.
    Project,
    /// Własna pamięć agentki (`MemoryScope::Agent(ja)`).
    Agent,
    /// Pamięć globalna.
    Global,
}

/// Dostęp agentki: tożsamość, sesja, projekt sesji i przyznane zakresy (z manifestu roli; Broker).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentAccess {
    /// Agentka.
    pub agent: AgentId,
    /// Sesja, w której pracuje.
    pub session: SessionId,
    /// Projekt sesji (`None` = sesja poza projektem).
    #[serde(default)]
    pub project: Option<String>,
    /// Zakresy do odczytu (`recall`, `get`).
    pub read: Vec<ScopeGrant>,
    /// Zakresy do zapisu (`remember`, przypięcie, zapomnienie własnych wpisów).
    pub write: Vec<ScopeGrant>,
}

impl AgentAccess {
    /// Domyślny dostęp: tylko własna sesja (odczyt i zapis).
    pub fn session_only(agent: AgentId, session: SessionId) -> Self {
        Self {
            agent,
            session,
            project: None,
            read: vec![ScopeGrant::Session],
            write: vec![ScopeGrant::Session],
        }
    }

    /// Tylko odczyt własnej sesji (mosty CLI: wyłącznie `recall`, PLAN §8.5).
    pub fn read_only(agent: AgentId, session: SessionId) -> Self {
        Self {
            write: Vec::new(),
            ..Self::session_only(agent, session)
        }
    }

    /// Konkretny zakres odpowiadający przyznaniu (`None`, gdy przyznanie nie ma celu, np. projekt
    /// dla sesji poza projektem).
    pub fn resolve(&self, grant: ScopeGrant) -> Option<MemoryScope> {
        match grant {
            ScopeGrant::Session => Some(MemoryScope::Session(self.session.clone())),
            ScopeGrant::Project => self.project.clone().map(MemoryScope::Project),
            ScopeGrant::Agent => Some(MemoryScope::Agent(self.agent.clone())),
            ScopeGrant::Global => Some(MemoryScope::Global),
        }
    }

    fn allows(&self, grants: &[ScopeGrant], scope: &MemoryScope) -> bool {
        grants
            .iter()
            .filter_map(|g| self.resolve(*g))
            .any(|s| s == *scope)
    }
}

/// Wywołujący operację pamięci.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "accessor", rename_all = "snake_case")]
pub enum Accessor {
    /// Właściciel przez UI (Inspektor, „zapamiętaj”, wyszukiwanie) — wszystkie zakresy.
    Owner,
    /// Strażniczka pamięci (konsolidacja w tle) — odczyt wszystkiego, zapis tylko przez
    /// [`crate::MemoryService::apply_changes`] i propozycje awansu (oczekujące).
    Guardian,
    /// Agentka w sesji z przyznanymi zakresami.
    Agent(AgentAccess),
}

impl Accessor {
    /// Klucz do pamięci podręcznej `recall` (bez treści).
    pub fn cache_key(&self) -> String {
        match self {
            Accessor::Owner => "owner".into(),
            Accessor::Guardian => "guardian".into(),
            Accessor::Agent(a) => format!(
                "agent:{}:{}:{:?}:{:?}:{:?}",
                a.agent, a.session, a.project, a.read, a.write
            ),
        }
    }

    /// Czy to właściciel.
    pub fn is_owner(&self) -> bool {
        matches!(self, Accessor::Owner)
    }
}

/// Rodzaj dostępu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// Odczyt.
    Read,
    /// Zapis.
    Write,
}

/// Sprawdza dostęp do zakresu. Właściciel — wszystko; Strażniczka — odczyt wszystkiego i zapis
/// (silnik ogranicza jej zapis do zmian konsolidacji); agentka — tylko przyznane zakresy.
pub fn authorize(who: &Accessor, scope: &MemoryScope, op: Op) -> Result<(), MemoryError> {
    match who {
        Accessor::Owner | Accessor::Guardian => Ok(()),
        Accessor::Agent(access) => {
            let grants = match op {
                Op::Read => &access.read,
                Op::Write => &access.write,
            };
            if access.allows(grants, scope) {
                Ok(())
            } else {
                Err(MemoryError::forbidden(format!(
                    "agentka {} nie ma dostępu ({op:?}) do zakresu {}",
                    access.agent,
                    crate::scope_key(scope)
                )))
            }
        }
    }
}

/// Zakresy czytelne dla agentki (domyślny zestaw `recall`), bez duplikatów.
pub fn readable_scopes(access: &AgentAccess) -> Vec<MemoryScope> {
    let mut out: Vec<MemoryScope> = Vec::new();
    for scope in access.read.iter().filter_map(|g| access.resolve(*g)) {
        if !out.contains(&scope) {
            out.push(scope);
        }
    }
    out
}

/// Wymaga właściciela (funkcje UI: Inspektor, edycja, eksport, import, zapomnienie zakresu).
pub fn require_owner(who: &Accessor, what: &str) -> Result<(), MemoryError> {
    if who.is_owner() {
        Ok(())
    } else {
        Err(MemoryError::forbidden(format!(
            "{what} — tylko właściciel (UI)"
        )))
    }
}

/// Wymaga właściciela albo Strażniczki (odczyt Inspektora przez konsolidację, zmiany konsolidacji).
pub fn require_owner_or_guardian(who: &Accessor, what: &str) -> Result<(), MemoryError> {
    match who {
        Accessor::Owner | Accessor::Guardian => Ok(()),
        Accessor::Agent(_) => Err(MemoryError::forbidden(format!(
            "{what} — niedostępne dla agentek"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent() -> AgentAccess {
        AgentAccess {
            project: Some("dom".into()),
            read: vec![ScopeGrant::Session, ScopeGrant::Project, ScopeGrant::Global],
            write: vec![ScopeGrant::Session, ScopeGrant::Agent],
            ..AgentAccess::session_only(AgentId::new("beta"), SessionId::new("A"))
        }
    }

    #[test]
    fn grants_resolve_relative_to_agent() {
        let who = Accessor::Agent(agent());
        let a = MemoryScope::Session(SessionId::new("A"));
        let b = MemoryScope::Session(SessionId::new("B"));
        assert!(authorize(&who, &a, Op::Read).is_ok());
        assert!(authorize(&who, &b, Op::Read).is_err());
        assert!(authorize(&who, &MemoryScope::Project("dom".into()), Op::Read).is_ok());
        assert!(authorize(&who, &MemoryScope::Project("praca".into()), Op::Read).is_err());
        assert!(authorize(&who, &MemoryScope::Global, Op::Read).is_ok());
        assert!(authorize(&who, &MemoryScope::Global, Op::Write).is_err());
        let own = MemoryScope::Agent(AgentId::new("beta"));
        assert!(
            authorize(&who, &own, Op::Write).is_ok() && authorize(&who, &own, Op::Read).is_err()
        );
        let other = MemoryScope::Agent(AgentId::new("alfa"));
        assert!(authorize(&who, &other, Op::Write).is_err());
        assert_eq!(readable_scopes(&agent()).len(), 3);
        assert!(authorize(&Accessor::Owner, &b, Op::Write).is_ok());
        let cli = AgentAccess::read_only(AgentId::new("delta"), SessionId::new("A"));
        assert!(authorize(&Accessor::Agent(cli.clone()), &a, Op::Write).is_err());
        assert!(
            AgentAccess {
                project: None,
                ..cli
            }
            .resolve(ScopeGrant::Project)
            .is_none()
        );
        assert!(
            require_owner(&who, "x").is_err()
                && require_owner_or_guardian(&Accessor::Guardian, "x").is_ok()
        );
    }
}
