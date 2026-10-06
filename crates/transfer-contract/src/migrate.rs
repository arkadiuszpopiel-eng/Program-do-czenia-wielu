//! Upcastery (docs/formats/alfa-package.md §7): łańcuch vN → vN+1 dla manifestu, nagłówka sesji
//! i rekordów tur. Paczka może mieszać wersje rekordów; nowsze od obsługiwanych są odrzucane.
//!
//! **v0** — szkic formatu sprzed SPEC v1 (paczki testowe i wczesne eksporty):
//! manifest `{ format: 0, app, created, machine, kind?, files: [{ name, sha256, size }], notes? }`,
//! nagłówek sesji `{ v: 0, id, title, created }`, tura `{ v: 0, id, parent, role, text, ts, agent? }`
//! (bez gałęzi i autora — wyliczane przy migracji regułami `TreeCursor`).

use std::collections::BTreeSet;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sessions_contract::{
    AgentId, Author, PrivacyTag, Role, SessionId, SessionMeta, SessionTemplate, TreeCursor, Turn,
    TurnContent, TurnId,
};

use crate::error::TransferError;
use crate::manifest::{
    ContentEntry, MachineInfo, Manifest, PackageKind, ScopeSummary, check_schema_version,
    content_sha256, schema_version, sha256_hex,
};
use crate::paths::{EntryKind, MANIFEST_PATH, SESSION_FILE, classify};
use crate::portable::{SESSION_RECORD_VERSION, SessionHeader, decode_turn_v1};
use crate::report::UpcastStep;

#[derive(Deserialize)]
struct FileV0 {
    name: String,
    sha256: String,
    size: u64,
}

#[derive(Deserialize)]
struct ManifestV0 {
    format: u32,
    app: String,
    created: DateTime<Utc>,
    machine: String,
    kind: Option<PackageKind>,
    files: Vec<FileV0>,
    notes: Option<String>,
}

fn step(entity: &str, from: &str, to: &str, count: u64) -> UpcastStep {
    UpcastStep {
        entity: entity.to_owned(),
        from: from.to_owned(),
        to: to.to_owned(),
        count,
    }
}

/// Manifest w dowolnej obsługiwanej wersji → manifest v1 (+ zastosowane kroki). Wersję
/// `schema_version` sprawdza **przed** odczytem struktury ([`check_schema_version`]): manifest
/// innego major ma inną strukturę, więc odmowa „nowsza”/„za stara” zamiast „nieprawidłowe dane”
/// (fala 5, m-06). Pełną walidację robi potem [`Manifest::validate`].
pub fn upcast_manifest(
    value: serde_json::Value,
) -> Result<(Manifest, Vec<UpcastStep>), TransferError> {
    let is_v0 = value.get("schema_version").is_none()
        && value.get("format").and_then(serde_json::Value::as_u64) == Some(0);
    if !is_v0 {
        if let Some(found) = value
            .get("schema_version")
            .and_then(serde_json::Value::as_str)
            .and_then(|v| semver::Version::parse(v).ok())
        {
            check_schema_version(&found)?;
        }
        let manifest = serde_json::from_value::<Manifest>(value)
            .map_err(|e| TransferError::invalid(MANIFEST_PATH, e))?;
        return Ok((manifest, Vec::new()));
    }
    let v0: ManifestV0 =
        serde_json::from_value(value).map_err(|e| TransferError::invalid(MANIFEST_PATH, e))?;
    if v0.format != 0 {
        return Err(TransferError::invalid(MANIFEST_PATH, "nieznany format"));
    }
    let content: Vec<ContentEntry> = v0
        .files
        .into_iter()
        .map(|f| ContentEntry {
            path: f.name,
            sha256: f.sha256,
            bytes: f.size,
        })
        .collect();
    let mut keys = BTreeSet::new();
    let mut sessions = BTreeSet::new();
    for entry in &content {
        match classify(&entry.path) {
            EntryKind::Document { category, .. } => {
                keys.insert(category.key().to_owned());
            }
            EntryKind::SessionFile { id, file } => {
                keys.insert("sessions".to_owned());
                if file == SESSION_FILE {
                    sessions.insert(id);
                }
            }
            _ => {}
        }
    }
    let mut scope = ScopeSummary {
        keys: keys.into_iter().collect(),
        sessions: sessions.into_iter().collect(),
        ..ScopeSummary::default()
    };
    scope.counts.sessions = scope.sessions.len() as u64;
    let manifest = Manifest {
        schema_version: schema_version(),
        app_version: semver::Version::parse(&v0.app).unwrap_or(semver::Version::new(0, 0, 0)),
        kind: v0.kind.unwrap_or(PackageKind::Export),
        created_at: v0.created,
        source_machine: MachineInfo {
            id: v0.machine,
            ..MachineInfo::default()
        },
        scope,
        content_sha256: content_sha256(&content),
        content,
        encryption: None,
        notes: v0.notes,
        redactions: 0,
    };
    Ok((manifest, vec![step("manifest", "0", "1.0.0", 1)]))
}

#[derive(Deserialize)]
struct HeaderV0 {
    id: SessionId,
    title: String,
    created: DateTime<Utc>,
}

fn record_version(value: &serde_json::Value) -> u64 {
    value
        .get("v")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

fn newer(where_: &str, found: u64) -> TransferError {
    TransferError::NewerSchema {
        found: format!("{where_}: rekord v{found}"),
        supported: format!("v{SESSION_RECORD_VERSION}"),
    }
}

/// Nagłówek sesji dowolnej obsługiwanej wersji → v1.
pub fn upcast_session_header(
    raw: serde_json::Value,
    turns: &[u8],
    where_: &str,
) -> Result<(SessionHeader, Vec<UpcastStep>), TransferError> {
    match record_version(&raw) {
        1 => serde_json::from_value(raw)
            .map(|h| (h, Vec::new()))
            .map_err(|e| TransferError::invalid(where_, e)),
        0 => {
            let v0: HeaderV0 =
                serde_json::from_value(raw).map_err(|e| TransferError::invalid(where_, e))?;
            let count = turns
                .split(|b| *b == b'\n')
                .filter(|l| !l.iter().all(u8::is_ascii_whitespace))
                .count() as u64;
            let meta = SessionMeta {
                workdir: PathBuf::from(v0.id.as_str()),
                id: v0.id.clone(),
                title: v0.title,
                template: SessionTemplate::Empty,
                model_policy: "auto".to_owned(),
                agents: Vec::new(),
                privacy: PrivacyTag::Normal,
                tainted: false,
                pinned: false,
                archived: false,
                trashed: false,
                project: None,
                tags: Vec::new(),
                created_at: v0.created,
                updated_at: v0.created,
            };
            let header = SessionHeader {
                v: SESSION_RECORD_VERSION,
                meta,
                workdir_rel: Some(v0.id.as_str().to_owned()),
                active_leaf: (count > 0).then_some(TurnId(count)),
                draft: None,
                turns: count,
                turns_sha256: sha256_hex(turns),
            };
            Ok((header, vec![step("session", "0", "1", 1)]))
        }
        other => Err(newer(where_, other)),
    }
}

#[derive(Deserialize)]
struct TurnV0 {
    id: TurnId,
    parent: Option<TurnId>,
    role: Role,
    text: String,
    ts: DateTime<Utc>,
    agent: Option<String>,
}

fn upcast_turn_v0(v0: TurnV0, cursor: &TreeCursor) -> Result<Turn, TransferError> {
    let author = match v0.role {
        Role::User => Author::User,
        Role::Assistant => Author::Agent {
            agent: AgentId::new(v0.agent.as_deref().unwrap_or("alfa")),
        },
        Role::System => Author::System,
        Role::Tool => Author::Tool {
            name: "narzędzie".to_owned(),
        },
    };
    let (branch, _) = cursor
        .branch_for(v0.parent)
        .map_err(|e| TransferError::invalid("turns", e))?;
    Ok(Turn {
        id: v0.id,
        parent: v0.parent,
        branch,
        role: v0.role,
        author,
        content: TurnContent::text(v0.text),
        usage: None,
        created_at: v0.ts,
        heard_prefix: None,
        hidden: false,
    })
}

/// Dekoduje `turns.ndjson` (każda linia z własną wersją) → tury v1 w kolejności pliku.
pub fn decode_turns(
    bytes: &[u8],
    where_: &str,
) -> Result<(Vec<Turn>, Vec<UpcastStep>), TransferError> {
    let text = std::str::from_utf8(bytes).map_err(|e| TransferError::invalid(where_, e))?;
    let mut cursor = TreeCursor::empty();
    let mut turns = Vec::new();
    let mut upcast = 0;
    for (n, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let bad = |e: &dyn std::fmt::Display| {
            TransferError::invalid(format!("{where_}/turns.ndjson:{}", n + 1), e)
        };
        let value: serde_json::Value = serde_json::from_str(line).map_err(|e| bad(&e))?;
        let turn = match record_version(&value) {
            1 => decode_turn_v1(value).map_err(|e| bad(&e))?,
            0 => {
                upcast += 1;
                let v0: TurnV0 = serde_json::from_value(value).map_err(|e| bad(&e))?;
                upcast_turn_v0(v0, &cursor)?
            }
            other => return Err(newer(where_, other)),
        };
        cursor.accept(&turn).map_err(|e| bad(&e))?;
        turns.push(turn);
    }
    let steps = if upcast > 0 {
        vec![step("turn", "0", "1", upcast)]
    } else {
        Vec::new()
    };
    Ok((turns, steps))
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
