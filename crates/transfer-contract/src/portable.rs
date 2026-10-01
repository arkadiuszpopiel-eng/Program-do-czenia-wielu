//! Format przenośny sesji w paczce: `sessions/<id>/session.json` (metadane, liść, szkic) i
//! `sessions/<id>/turns.ndjson` (drzewo tur z gałęziami, jedna tura na linię, wersja rekordu w
//! każdej linii). Nie zawiera surowych plików SQLCipher (klucz bazy jest per maszyna).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sessions_contract::{PortableSession, SessionId, SessionMeta, Turn, TurnId};

use crate::error::TransferError;
use crate::manifest::sha256_hex;
use crate::migrate;
use crate::paths::validate_entry_path;
use crate::report::UpcastStep;

/// Wersja rekordów sesji zapisywana przez tę wersję Alfy.
pub const SESSION_RECORD_VERSION: u32 = 1;

/// `session.json` (v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionHeader {
    /// Wersja rekordu.
    pub v: u32,
    /// Metadane; `workdir` względny, gdy znany jest korzeń katalogów roboczych.
    pub meta: SessionMeta,
    /// Katalog roboczy względem korzenia (`/` jako separator; bez nazwy konta Windows).
    pub workdir_rel: Option<String>,
    /// Aktywny liść.
    pub active_leaf: Option<TurnId>,
    /// Szkic composera.
    pub draft: Option<String>,
    /// Liczba tur w `turns.ndjson`.
    pub turns: u64,
    /// SHA-256 pliku `turns.ndjson`.
    pub turns_sha256: String,
}

/// Linia `turns.ndjson` (v1): wersja + tura.
#[derive(Serialize)]
struct TurnRecord<'a> {
    v: u32,
    #[serde(flatten)]
    turn: std::borrow::Cow<'a, Turn>,
}

/// Zakodowana sesja: `(session.json, turns.ndjson)`.
pub fn encode_session(
    session: &PortableSession,
    workdir_root: Option<&Path>,
) -> Result<(Vec<u8>, Vec<u8>), TransferError> {
    let mut turns = Vec::new();
    for turn in &session.turns {
        let rec = TurnRecord {
            v: SESSION_RECORD_VERSION,
            turn: std::borrow::Cow::Borrowed(turn),
        };
        serde_json::to_writer(&mut turns, &rec).map_err(|e| TransferError::invalid("turns", e))?;
        turns.push(b'\n');
    }
    let mut meta = session.meta.clone();
    let workdir_rel = relative_workdir(&meta.workdir, workdir_root);
    if let Some(rel) = &workdir_rel {
        meta.workdir = PathBuf::from(rel);
    }
    let header = SessionHeader {
        v: SESSION_RECORD_VERSION,
        meta,
        workdir_rel,
        active_leaf: session.active_leaf,
        draft: session.draft.clone(),
        turns: session.turns.len() as u64,
        turns_sha256: sha256_hex(&turns),
    };
    let header =
        serde_json::to_vec_pretty(&header).map_err(|e| TransferError::invalid("session", e))?;
    Ok((header, turns))
}

/// Katalog roboczy względem korzenia; poza korzeniem — sama nazwa ostatniego katalogu.
fn relative_workdir(workdir: &Path, root: Option<&Path>) -> Option<String> {
    let root = root?;
    let rel = match workdir.strip_prefix(root) {
        Ok(rel) => rel.to_path_buf(),
        Err(_) => PathBuf::from(workdir.file_name()?),
    };
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let joined = parts.join("/");
    validate_entry_path(&joined).ok().map(|()| joined)
}

/// Dekoduje sesję z paczki (z migracją starszych rekordów); `id` = katalog w paczce.
pub fn decode_session(
    id: &SessionId,
    header: &[u8],
    turns: &[u8],
    workdir_root: Option<&Path>,
) -> Result<(PortableSession, Vec<UpcastStep>), TransferError> {
    let where_ = format!("sessions/{id}");
    let raw: serde_json::Value =
        serde_json::from_slice(header).map_err(|e| TransferError::invalid(&where_, e))?;
    let (header, mut steps) = migrate::upcast_session_header(raw, turns, &where_)?;
    if header.meta.id != *id {
        return Err(TransferError::invalid(
            &where_,
            "identyfikator niezgodny z katalogiem",
        ));
    }
    if header.turns_sha256 != sha256_hex(turns) {
        return Err(TransferError::Checksum {
            path: format!("{where_}/turns.ndjson"),
        });
    }
    let (turns, turn_steps) = migrate::decode_turns(turns, &where_)?;
    steps.extend(turn_steps);
    if turns.len() as u64 != header.turns {
        return Err(TransferError::invalid(
            &where_,
            "liczba tur niezgodna z nagłówkiem",
        ));
    }
    let mut meta = header.meta;
    meta.workdir = local_workdir(&meta, header.workdir_rel.as_deref(), workdir_root);
    let session = PortableSession {
        meta,
        turns,
        active_leaf: header.active_leaf,
        draft: header.draft,
    };
    session
        .validate()
        .map_err(|e| TransferError::invalid(&where_, e))?;
    Ok((session, steps))
}

/// Katalog roboczy na tej maszynie: `<korzeń>/<względny>`; względny spoza reguł ścieżek
/// (np. `..`) → `<korzeń>/<id>`; bez korzenia — bez zmian.
fn local_workdir(meta: &SessionMeta, rel: Option<&str>, root: Option<&Path>) -> PathBuf {
    let Some(root) = root else {
        return meta.workdir.clone();
    };
    match rel.filter(|r| validate_entry_path(r).is_ok()) {
        Some(rel) => rel.split('/').fold(root.to_path_buf(), |p, s| p.join(s)),
        None => root.join(meta.id.as_str()),
    }
}

/// Encodowanie linii tury v1 (dla testów migracji i narzędzi).
pub fn encode_turn_line(turn: &Turn) -> Result<String, TransferError> {
    serde_json::to_string(&TurnRecord {
        v: SESSION_RECORD_VERSION,
        turn: std::borrow::Cow::Borrowed(turn),
    })
    .map_err(|e| TransferError::invalid("turns", e))
}

/// Dekodowanie linii v1 (bez migracji; pole `v` jest pomijane).
pub(crate) fn decode_turn_v1(value: serde_json::Value) -> Result<Turn, serde_json::Error> {
    serde_json::from_value::<Turn>(value)
}

/// Najpóźniejszy czas tury (do sortowania i porównań).
pub fn latest(turns: &[Turn]) -> Option<DateTime<Utc>> {
    turns.iter().map(|t| t.created_at).max()
}

#[cfg(test)]
#[path = "portable_tests.rs"]
mod tests;
