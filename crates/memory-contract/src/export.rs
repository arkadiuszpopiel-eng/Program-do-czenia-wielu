//! Format pamięci w paczce `.alfa` (docs/formats/alfa-package.md: `memory/<zakres>.ndjson`):
//! jeden dokument NDJSON na zakres, linia = wpis ([`MemoryEntry`] z proweniencją i wersjami).
//!
//! Nazwy dokumentów (względne wobec katalogu `memory/`): `global.ndjson`,
//! `project/<id>.ndjson`, `agent/<id>.ndjson`, `session/<id>.ndjson`.

use crate::error::MemoryError;
use crate::model::validate_scope;
use crate::types::{MemoryEntry, MemoryScope};

/// Rozszerzenie dokumentu.
pub const DOC_EXT: &str = ".ndjson";

/// Nazwa dokumentu zakresu.
pub fn document_name(scope: &MemoryScope) -> String {
    match scope {
        MemoryScope::Global => format!("global{DOC_EXT}"),
        MemoryScope::Project(p) => format!("project/{p}{DOC_EXT}"),
        MemoryScope::Agent(a) => format!("agent/{a}{DOC_EXT}"),
        MemoryScope::Session(s) => format!("session/{s}{DOC_EXT}"),
    }
}

/// Zakres z nazwy dokumentu (`None` — nazwa spoza formatu lub identyfikator nieprzenośny).
pub fn scope_of_document(name: &str) -> Option<MemoryScope> {
    let stem = name.strip_suffix(DOC_EXT)?;
    let scope = match stem.split_once('/') {
        None if stem == "global" => MemoryScope::Global,
        Some(("project", id)) => MemoryScope::Project(id.to_owned()),
        Some(("agent", id)) => MemoryScope::Agent(core_bus_contract::AgentId::new(id)),
        Some(("session", id)) => MemoryScope::Session(core_bus_contract::SessionId::new(id)),
        _ => return None,
    };
    let portable = |id: &str| {
        !id.is_empty()
            && id.len() <= 64
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    let id_ok = match &scope {
        MemoryScope::Global => true,
        MemoryScope::Project(p) => portable(p),
        MemoryScope::Agent(a) => portable(a.as_str()),
        MemoryScope::Session(s) => portable(s.as_str()),
    };
    (id_ok && validate_scope(&scope).is_ok()).then_some(scope)
}

/// Koduje wpisy jako NDJSON (linia na wpis, `\n` na końcu każdej).
pub fn encode_ndjson(entries: &[MemoryEntry]) -> Result<Vec<u8>, MemoryError> {
    let mut out = Vec::new();
    for e in entries {
        let line = serde_json::to_vec(e).map_err(MemoryError::storage)?;
        out.extend_from_slice(&line);
        out.push(b'\n');
    }
    Ok(out)
}

/// Dekoduje NDJSON (puste linie pomijane); błąd wskazuje numer linii (od 1).
pub fn decode_ndjson(bytes: &[u8]) -> Result<Vec<MemoryEntry>, MemoryError> {
    let text = std::str::from_utf8(bytes).map_err(|e| MemoryError::invalid(e.to_string()))?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry = serde_json::from_str(line)
            .map_err(|e| MemoryError::invalid(format!("linia {}: {e}", i + 1)))?;
        out.push(entry);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_bus_contract::{AgentId, SessionId};

    #[test]
    fn names_round_trip_and_reject_traversal() {
        for scope in [
            MemoryScope::Global,
            MemoryScope::Project("dom".into()),
            MemoryScope::Agent(AgentId::new("beta")),
            MemoryScope::Session(SessionId::new("s-1")),
        ] {
            assert_eq!(scope_of_document(&document_name(&scope)), Some(scope));
        }
        for bad in [
            "global.json",
            "project/../x.ndjson",
            "session/.ndjson",
            "x/y.ndjson",
            "project/a/b.ndjson",
        ] {
            assert_eq!(scope_of_document(bad), None, "{bad}");
        }
        assert!(decode_ndjson(b"\n\n").unwrap().is_empty());
        let err = decode_ndjson(b"{}\n").unwrap_err();
        assert!(err.to_string().contains("linia 1"));
    }
}
