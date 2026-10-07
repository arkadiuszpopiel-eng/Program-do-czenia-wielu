//! Dostęp agentki do pamięci z obsady (PLAN §9.2: „uprawnienia idą za rolą") i projektu sesji
//! (PLAN §9.5: `memory: { scope, retain }` w manifeście roli). Do czasu pola `memory` w rolach
//! `personas` zakresy wynikają z tabeli ról poniżej (wariant ostrożny: zapis poza sesją i własną
//! pamięcią agentki tylko dla Strażniczki — i nawet wtedy jako wpis oczekujący na zgodę).
//!
//! Przegląd #2, P2-06 (wariant ściślejszy): narzędzia pamięci w przebiegu dostają zakresy **roli
//! bieżącego przebiegu** (`ToolCtx::holder.role`) ∩ role persony w obsadzie — nie sumę wszystkich
//! ról persony. Badaczka (treść niezaufana) zostaje w sesji, nawet gdy ta sama persona gra też
//! Krytyczkę czy Myślicielkę. Rola spoza obsady albo jej brak = tylko sesja (fail-closed).

use std::collections::BTreeSet;
use std::sync::Arc;

use memory_contract::{AgentAccess, AgentId, ScopeGrant, SessionId};
use personas_contract::{PersonaId, Personas, RoleId};
use sessions_contract::SessionCatalog;

use crate::ids::project_slug;

/// Zakresy roli: (odczyt, zapis).
pub fn role_grants(role: &str) -> (Vec<ScopeGrant>, Vec<ScopeGrant>) {
    use ScopeGrant::{Agent, Global, Project, Session};
    match role {
        // Strażniczka pamięci: wszystko; zapis szerszy niż sesja = oczekujący (zgoda użytkownika).
        "keeper" => (
            vec![Session, Project, Agent, Global],
            vec![Session, Project, Agent, Global],
        ),
        "conductor" | "speaker" => (vec![Session, Project, Agent, Global], vec![Session, Agent]),
        // Krytyczka: tylko odczyt.
        "critic" => (vec![Session, Project], Vec::new()),
        // Badaczka pracuje na treści niezaufanej — wyłącznie własna sesja.
        "researcher" => (vec![Session], vec![Session]),
        _ => (vec![Session, Project, Agent], vec![Session, Agent]),
    }
}

/// Rozwiązuje dostęp agentki w sesji (role z obsady sesji, projekt z katalogu).
pub struct RoleAccess {
    personas: Arc<dyn Personas>,
    catalog: Arc<dyn SessionCatalog>,
}

impl RoleAccess {
    /// Nowy resolver.
    pub fn new(personas: Arc<dyn Personas>, catalog: Arc<dyn SessionCatalog>) -> Self {
        Self { personas, catalog }
    }

    /// Projekt sesji jako identyfikator zakresu pamięci (`None` — sesja poza projektem).
    pub fn project_of(&self, session: &SessionId) -> Option<String> {
        let meta = self.catalog.session(session).ok()?;
        meta.project.as_ref().and_then(|p| project_slug(&p.0))
    }

    /// Dostęp agentki w rozmowie (kontekst pamięci czatu): suma zakresów jej ról w obsadzie
    /// sesji (bez ról — tylko sesja).
    pub fn access(&self, session: &SessionId, agent: &str) -> AgentAccess {
        let roles = self.personas.cast(session).roles_of(&PersonaId::new(agent));
        self.build(session, agent, sum_grants(&roles))
    }

    /// Dostęp narzędzi pamięci w przebiegu: rola przebiegu ∩ role persony (P2-06).
    pub fn access_for_run(
        &self,
        session: &SessionId,
        agent: &str,
        run_role: Option<&str>,
    ) -> AgentAccess {
        let roles = self.personas.cast(session).roles_of(&PersonaId::new(agent));
        self.build(session, agent, run_grants(&roles, run_role))
    }

    fn build(
        &self,
        session: &SessionId,
        agent: &str,
        (read, write): (Vec<ScopeGrant>, Vec<ScopeGrant>),
    ) -> AgentAccess {
        AgentAccess {
            agent: AgentId::new(agent),
            session: session.clone(),
            project: self.project_of(session),
            read,
            write,
        }
    }
}

/// Suma zakresów ról (sesja zawsze do odczytu; bez ról — sesja także do zapisu).
pub fn sum_grants(roles: &BTreeSet<RoleId>) -> (Vec<ScopeGrant>, Vec<ScopeGrant>) {
    let mut read = BTreeSet::new();
    let mut write = BTreeSet::new();
    read.insert(grant_key(ScopeGrant::Session));
    for role in roles {
        let (r, w) = role_grants(role.as_str());
        read.extend(r.into_iter().map(grant_key));
        write.extend(w.into_iter().map(grant_key));
    }
    if roles.is_empty() {
        write.insert(grant_key(ScopeGrant::Session));
    }
    (
        read.into_iter().map(grant_of).collect(),
        write.into_iter().map(grant_of).collect(),
    )
}

/// Zakresy przebiegu: tylko rola bieżącego przebiegu, o ile persona ją gra w obsadzie; inaczej
/// (rola nieznana, spoza obsady, brak roli) — wyłącznie sesja (P2-06, wariant ściślejszy).
pub fn run_grants(
    roles: &BTreeSet<RoleId>,
    run_role: Option<&str>,
) -> (Vec<ScopeGrant>, Vec<ScopeGrant>) {
    match run_role.map(str::trim) {
        Some(role) if roles.iter().any(|r| r.as_str() == role) => {
            let one: BTreeSet<RoleId> = [RoleId::new(role)].into();
            sum_grants(&one)
        }
        _ => (vec![ScopeGrant::Session], vec![ScopeGrant::Session]),
    }
}

fn grant_key(g: ScopeGrant) -> u8 {
    match g {
        ScopeGrant::Session => 0,
        ScopeGrant::Project => 1,
        ScopeGrant::Agent => 2,
        ScopeGrant::Global => 3,
    }
}

fn grant_of(k: u8) -> ScopeGrant {
    match k {
        0 => ScopeGrant::Session,
        1 => ScopeGrant::Project,
        2 => ScopeGrant::Agent,
        _ => ScopeGrant::Global,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critic_reads_only_and_researcher_stays_in_session() {
        let (r, w) = role_grants("critic");
        assert!(w.is_empty() && r.contains(&ScopeGrant::Project));
        let (r, w) = role_grants("researcher");
        assert_eq!(
            (r, w),
            (vec![ScopeGrant::Session], vec![ScopeGrant::Session])
        );
        let (_, w) = role_grants("operator");
        assert!(!w.contains(&ScopeGrant::Global) && !w.contains(&ScopeGrant::Project));
        for k in 0..4 {
            assert_eq!(grant_key(grant_of(k)), k);
        }
    }

    /// P2-06: Gama gra Badaczkę, Krytyczkę i Myślicielkę — w przebiegu Badaczki (treść
    /// niezaufana) suma ról dawała odczyt projektu i własnej pamięci oraz zapis do niej.
    #[test]
    fn run_role_not_sum_of_persona_roles() {
        let gama: BTreeSet<RoleId> = ["researcher", "critic", "thinker"]
            .into_iter()
            .map(RoleId::new)
            .collect();
        let (sum_read, sum_write) = sum_grants(&gama);
        assert!(sum_read.contains(&ScopeGrant::Project) && sum_write.contains(&ScopeGrant::Agent));
        let only_session = (vec![ScopeGrant::Session], vec![ScopeGrant::Session]);
        assert_eq!(run_grants(&gama, Some("researcher")), only_session);
        let (r, w) = run_grants(&gama, Some("critic"));
        assert!(r.contains(&ScopeGrant::Project) && w.is_empty());
        // Rola spoza obsady persony, brak roli — tylko sesja.
        assert_eq!(run_grants(&gama, Some("keeper")), only_session);
        assert_eq!(run_grants(&gama, None), only_session);
    }
}
