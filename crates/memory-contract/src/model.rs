//! Model F7: odwołania do wpisów, pochodzenie (proweniencja szczegółowa), wersje i stany wpisów,
//! klucze zakresów (PLAN §10, docs/modules/memory/SPEC.md „F7”).

use std::fmt;

use chrono::{DateTime, Utc};
use core_bus_contract::SessionId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::{MemoryEntry, MemoryId, MemoryScope};

/// Odwołanie do wpisu: zakres (baza) + identyfikator.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct EntryRef {
    /// Zakres, w którym żyje wpis.
    pub scope: MemoryScope,
    /// Identyfikator.
    pub id: MemoryId,
}

impl EntryRef {
    /// Nowe odwołanie.
    pub fn new(scope: MemoryScope, id: MemoryId) -> Self {
        Self { scope, id }
    }
}

impl fmt::Display for EntryRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", scope_key(&self.scope), self.id)
    }
}

/// Jak powstał wpis pochodny (każdy rodzaj niesie treść źródeł — kaskada `forget` usuwa pochodną,
/// gdy zniknie którekolwiek źródło; duplikaty obsługuje zastąpienie + przywrócenie).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Derivation {
    /// Fakt wyekstrahowany z epizodów (konsolidacja).
    Extracted,
    /// Streszczenie epizodów.
    Summary,
    /// Umiejętność (warstwa proceduralna) wyuczona z epizodów.
    Skill,
    /// Kopia w zakresie szerszym (awans za zgodą).
    Promoted,
    /// Nowa wersja po edycji w Inspektorze.
    Edited,
    /// Import z paczki `.alfa`.
    Imported,
}

/// Pochodzenie wpisu: sesja i tura źródłowa, wpisy źródłowe, rodzaj wyprowadzenia.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Origin {
    /// Sesja, z której pochodzi treść (także dla wpisów w zakresach szerszych).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
    /// Numer tury w sesji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<u64>,
    /// Wpisy źródłowe (ten sam zakres albo węższy).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<EntryRef>,
    /// Rodzaj wyprowadzenia (`None` = wpis pierwotny).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation: Option<Derivation>,
}

impl Origin {
    /// Brak informacji o pochodzeniu (wpis v0).
    pub fn is_empty(&self) -> bool {
        self.session.is_none()
            && self.turn.is_none()
            && self.derived_from.is_empty()
            && self.derivation.is_none()
    }

    /// Pochodzenie z tury sesji.
    pub fn from_turn(session: SessionId, turn: u64) -> Self {
        Self {
            session: Some(session),
            turn: Some(turn),
            ..Self::default()
        }
    }

    /// Wpis pochodny.
    pub fn derived(derivation: Derivation, sources: Vec<EntryRef>) -> Self {
        Self {
            derived_from: sources,
            derivation: Some(derivation),
            ..Self::default()
        }
    }
}

/// Powód zastąpienia wpisu nowszym.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SupersedeReason {
    /// Edycja użytkownika (nowa wersja).
    Edit,
    /// Sprzeczność z nowszym faktem (nowa wersja z odwołaniem).
    Contradiction,
    /// Duplikat scalony z innym wpisem (deduplikacja).
    Duplicate,
}

/// Zastąpienie: wpis zostaje w historii, ale nie wraca w `recall`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Supersession {
    /// Wpis, który zastąpił (ten sam zakres).
    pub by: MemoryId,
    /// Powód.
    pub reason: SupersedeReason,
    /// Kiedy.
    pub at: DateTime<Utc>,
}

/// Stan wpisu (liczony, nie zapisywany).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    /// Aktywny: zatwierdzony, aktualny, niewygasły.
    Active,
    /// Czeka na zatwierdzenie użytkownika.
    Pending,
    /// Zastąpiony nowszą wersją lub scalony.
    Superseded,
    /// Wygasły (TTL) — czeka na usunięcie przez retencję.
    Expired,
}

/// Stan wpisu w chwili `now` (kolejność: zastąpiony → oczekujący → wygasły → aktywny).
pub fn entry_state(entry: &MemoryEntry, now: DateTime<Utc>) -> EntryState {
    if entry.superseded.is_some() {
        EntryState::Superseded
    } else if !entry.approved {
        EntryState::Pending
    } else if crate::rules::is_expired(entry, now) {
        EntryState::Expired
    } else {
        EntryState::Active
    }
}

/// Czy wpis może wrócić w `recall` (aktywny).
pub fn is_recallable(entry: &MemoryEntry, now: DateTime<Utc>) -> bool {
    entry_state(entry, now) == EntryState::Active
}

/// Moment wygaśnięcia (`created_at + ttl`), jeśli wpis ma TTL.
pub fn expires_at(entry: &MemoryEntry) -> Option<DateTime<Utc>> {
    let ttl = i64::try_from(entry.ttl_secs?).ok()?;
    entry
        .created_at
        .checked_add_signed(chrono::Duration::try_seconds(ttl)?)
}

/// Klucz tekstowy zakresu: `session:<id>`, `project:<id>`, `agent:<id>`, `global`.
pub fn scope_key(scope: &MemoryScope) -> String {
    match scope {
        MemoryScope::Session(s) => format!("session:{s}"),
        MemoryScope::Project(p) => format!("project:{p}"),
        MemoryScope::Agent(a) => format!("agent:{a}"),
        MemoryScope::Global => "global".into(),
    }
}

/// Odwrotność [`scope_key`]; `None` dla nieznanego formatu lub pustego identyfikatora.
pub fn parse_scope_key(key: &str) -> Option<MemoryScope> {
    if key == "global" {
        return Some(MemoryScope::Global);
    }
    let (kind, id) = key.split_once(':')?;
    if id.is_empty() {
        return None;
    }
    match kind {
        "session" => Some(MemoryScope::Session(SessionId::new(id))),
        "project" => Some(MemoryScope::Project(id.to_owned())),
        "agent" => Some(MemoryScope::Agent(core_bus_contract::AgentId::new(id))),
        _ => None,
    }
}

/// Sesja zakresu `Session`.
pub fn scope_session(scope: &MemoryScope) -> Option<&SessionId> {
    match scope {
        MemoryScope::Session(s) => Some(s),
        _ => None,
    }
}

/// Czy `wider` jest ściśle szerszy niż `narrower` (sesja < projekt/agentka < globalna).
pub fn is_broader(wider: &MemoryScope, narrower: &MemoryScope) -> bool {
    let rank = |s: &MemoryScope| match s {
        MemoryScope::Session(_) => 0,
        MemoryScope::Project(_) | MemoryScope::Agent(_) => 1,
        MemoryScope::Global => 2,
    };
    rank(wider) > rank(narrower)
}

/// Maksymalna długość identyfikatora projektu/agentki w zakresie (trafia do nazwy pliku bazy).
pub const MAX_SCOPE_ID_LEN: usize = 64;

/// Identyfikator projektu/agentki w zakresie: `[A-Za-z0-9_-]{1,64}` (nazwa pliku bazy zakresu);
/// sesja: niepusta, bez znaków sterujących.
pub fn validate_scope(scope: &MemoryScope) -> Result<(), crate::MemoryError> {
    let portable = |id: &str| {
        !id.is_empty()
            && id.len() <= MAX_SCOPE_ID_LEN
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    let ok = match scope {
        MemoryScope::Session(s) => {
            !s.as_str().is_empty() && !s.as_str().chars().any(char::is_control)
        }
        MemoryScope::Project(p) => portable(p),
        MemoryScope::Agent(a) => portable(a.as_str()),
        MemoryScope::Global => true,
    };
    if ok {
        Ok(())
    } else {
        Err(crate::MemoryError::Invalid {
            reason: format!("nieprawidłowy zakres `{}`", scope_key(scope)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_bus_contract::AgentId;

    #[test]
    fn scope_keys_round_trip_and_order() {
        let scopes = [
            MemoryScope::Session(SessionId::new("s-1")),
            MemoryScope::Project("p".into()),
            MemoryScope::Agent(AgentId::new("beta")),
            MemoryScope::Global,
        ];
        for s in &scopes {
            assert_eq!(parse_scope_key(&scope_key(s)).as_ref(), Some(s));
            assert!(validate_scope(s).is_ok());
        }
        assert!(parse_scope_key("project:").is_none() && parse_scope_key("x:y").is_none());
        assert!(is_broader(&scopes[3], &scopes[1]) && is_broader(&scopes[2], &scopes[0]));
        assert!(!is_broader(&scopes[1], &scopes[2]) && !is_broader(&scopes[0], &scopes[0]));
        assert!(validate_scope(&MemoryScope::Project("../x".into())).is_err());
        assert!(validate_scope(&MemoryScope::Agent(AgentId::new(""))).is_err());
        assert!(Origin::default().is_empty());
        assert!(!Origin::from_turn(SessionId::new("s"), 3).is_empty());
    }
}
