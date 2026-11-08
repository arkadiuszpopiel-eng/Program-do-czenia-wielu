//! Identyfikatory DTO pamięci: wpis `"<klucz zakresu>#<id>"`, zakres jako klucz
//! (`session:<id>`, `project:<id>`, `agent:<id>`, `global`) albo `MemoryScopeRef`, identyfikator
//! projektu pamięci z nazwy projektu sesji (`[a-z0-9_-]{1,64}`).

use app_api::AppError;
use app_api::dto::{MemoryScopeKind, MemoryScopeRef};
use memory_contract::{
    AgentId, EntryRef, MAX_SCOPE_ID_LEN, MemoryId, MemoryScope, SessionId, parse_scope_key,
    scope_key, validate_scope,
};

/// Identyfikator wpisu dla UI.
pub fn entry_dto(entry: &EntryRef) -> String {
    format!("{}#{}", scope_key(&entry.scope), entry.id)
}

/// Odwrotność [`entry_dto`] (identyfikator wpisu nie zawiera `#`).
pub fn parse_entry(id: &str) -> Result<EntryRef, AppError> {
    let bad = || AppError::invalid(format!("Nieprawidłowy identyfikator wpisu pamięci „{id}”."));
    let (scope, entry) = id.rsplit_once('#').ok_or_else(bad)?;
    if entry.is_empty() {
        return Err(bad());
    }
    let scope = parse_scope(scope)?;
    Ok(EntryRef::new(scope, MemoryId(entry.to_owned())))
}

/// Zakres z klucza (zwalidowany).
pub fn parse_scope(key: &str) -> Result<MemoryScope, AppError> {
    let scope = parse_scope_key(key)
        .ok_or_else(|| AppError::invalid(format!("Nieznany zakres pamięci „{key}”.")))?;
    validate_scope(&scope)?;
    Ok(scope)
}

/// Zakres → DTO.
pub fn scope_ref(scope: &MemoryScope) -> MemoryScopeRef {
    let (kind, id) = match scope {
        MemoryScope::Session(s) => (MemoryScopeKind::Session, Some(s.to_string())),
        MemoryScope::Project(p) => (MemoryScopeKind::Project, Some(p.clone())),
        MemoryScope::Agent(a) => (MemoryScopeKind::Agent, Some(a.to_string())),
        MemoryScope::Global => (MemoryScopeKind::Global, None),
    };
    MemoryScopeRef { kind, id }
}

/// DTO → zakres (zwalidowany).
pub fn scope_of(r: &MemoryScopeRef) -> Result<MemoryScope, AppError> {
    let id = || {
        r.id.clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::invalid("Zakres pamięci wymaga identyfikatora."))
    };
    let scope = match r.kind {
        MemoryScopeKind::Session => MemoryScope::Session(SessionId::new(id()?)),
        MemoryScopeKind::Project => MemoryScope::Project(id()?),
        MemoryScopeKind::Agent => MemoryScope::Agent(AgentId::new(id()?)),
        MemoryScopeKind::Global => MemoryScope::Global,
    };
    validate_scope(&scope)?;
    Ok(scope)
}

/// Identyfikator projektu w pamięci z nazwy projektu sesji: małe litery, znaki spoza
/// `[a-z0-9_-]` → `-` (polskie litery bez ogonków), najwyżej 64 znaki; `None` dla pustego.
pub fn project_slug(project: &str) -> Option<String> {
    let mut out = String::new();
    for c in project.trim().to_lowercase().chars() {
        let mapped = match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => c,
            _ => '-',
        };
        if !(mapped == '-' && out.ends_with('-')) {
            out.push(mapped);
        }
    }
    let out: String = out
        .trim_matches('-')
        .chars()
        .take(MAX_SCOPE_ID_LEN)
        .collect();
    (!out.is_empty()).then_some(out)
}

/// Etykieta zakresu (PL) dla listy Inspektora.
pub fn scope_label(scope: &MemoryScope, session_title: Option<&str>) -> String {
    match scope {
        MemoryScope::Session(s) => match session_title {
            Some(t) => format!("Sesja: {t}"),
            None => format!("Sesja {s}"),
        },
        MemoryScope::Project(p) => format!("Projekt: {p}"),
        MemoryScope::Agent(a) => {
            let name = a.as_str();
            let mut chars = name.chars();
            let cap = chars
                .next()
                .map(|f| f.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default();
            format!("Agentka: {cap}")
        }
        MemoryScope::Global => "Pamięć globalna".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_ids_round_trip() {
        for scope in [
            MemoryScope::Session(SessionId::new("s-1")),
            MemoryScope::Project("dom".into()),
            MemoryScope::Agent(AgentId::new("beta")),
            MemoryScope::Global,
        ] {
            let r = EntryRef::new(scope.clone(), MemoryId("mem-1".into()));
            assert_eq!(parse_entry(&entry_dto(&r)).unwrap(), r);
            assert_eq!(scope_of(&scope_ref(&scope)).unwrap(), scope);
        }
        assert!(parse_entry("global").is_err() && parse_entry("global#").is_err());
        assert!(parse_entry("project:../x#m").is_err());
        assert!(parse_entry("nieznany:x#m").is_err());
    }

    #[test]
    fn project_slugs_are_valid_scope_ids() {
        assert_eq!(
            project_slug("Projekt Żółć 2026!").as_deref(),
            Some("projekt-zolc-2026")
        );
        assert_eq!(project_slug("  ").as_deref(), None);
        let long = "a".repeat(100);
        assert_eq!(project_slug(&long).map(|s| s.len()), Some(64));
        assert_eq!(
            scope_label(&MemoryScope::Agent(AgentId::new("beta")), None),
            "Agentka: Beta"
        );
    }
}
