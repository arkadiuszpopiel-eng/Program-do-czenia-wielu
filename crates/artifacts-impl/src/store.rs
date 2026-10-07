//! Tabele rejestru w bazie sesji; wersje niezmienne (wyzwalacze), migawki treści jako BLOB.

use artifacts_contract::{
    Artifact, ArtifactError, ArtifactId, ArtifactVersion, FileFacts, Origin, SessionId, TurnId,
    next_version,
};
use chrono::Utc;
use lib_sqlstore::migrate;
use lib_sqlstore::rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

/// Przestrzeń nazw migracji.
pub const NAMESPACE: &str = "artifacts";

/// Migracje: artefakty (kolejność rejestracji = `seq`) i wersje (metadane JSON + migawka).
pub const MIGRATIONS: &[(&str, &str)] = &[(
    "0001",
    "CREATE TABLE artifacts(
        seq INTEGER PRIMARY KEY,
        id TEXT NOT NULL UNIQUE,
        name TEXT NOT NULL,
        path TEXT NOT NULL,
        origin TEXT NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX artifacts_by_path ON artifacts(path);
    CREATE TABLE artifact_versions(
        artifact_id TEXT NOT NULL REFERENCES artifacts(id),
        version INTEGER NOT NULL,
        meta TEXT NOT NULL,
        snapshot BLOB,
        PRIMARY KEY(artifact_id, version)
    ) WITHOUT ROWID;
    CREATE TRIGGER artifact_versions_no_update BEFORE UPDATE ON artifact_versions
        BEGIN SELECT RAISE(ABORT, 'artifact_versions: wersje niezmienne'); END;
    CREATE TRIGGER artifact_versions_no_delete BEFORE DELETE ON artifact_versions
        BEGIN SELECT RAISE(ABORT, 'artifact_versions: wersje niezmienne'); END;",
)];

pub fn storage(e: impl std::fmt::Display) -> ArtifactError {
    ArtifactError::storage(e)
}

pub fn prepare(conn: &Connection) -> Result<(), ArtifactError> {
    migrate(conn, NAMESPACE, MIGRATIONS)
        .map(|_| ())
        .map_err(storage)
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Artefakt o danej ścieżce w sesji.
pub fn find_by_path(conn: &Connection, path: &Path) -> Result<Option<ArtifactId>, ArtifactError> {
    conn.query_row(
        "SELECT id FROM artifacts WHERE path = ?1 ORDER BY seq LIMIT 1",
        params![path_key(path)],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map(|id| id.map(ArtifactId))
    .map_err(storage)
}

/// Nowy artefakt (bez wersji).
pub fn insert_artifact(
    conn: &Connection,
    id: &ArtifactId,
    name: &str,
    path: &Path,
    origin: &Origin,
) -> Result<(), ArtifactError> {
    let origin = serde_json::to_string(origin).map_err(storage)?;
    conn.execute(
        "INSERT INTO artifacts(id, name, path, origin, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id.0,
            name,
            path_key(path),
            origin,
            Utc::now().timestamp_millis()
        ],
    )
    .map(|_| ())
    .map_err(storage)
}

/// Artefakt z wersjami.
pub fn load(
    conn: &Connection,
    session: &SessionId,
    id: &ArtifactId,
) -> Result<Artifact, ArtifactError> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT name, origin FROM artifacts WHERE id = ?1",
            params![id.0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    let (name, origin) = row.ok_or_else(|| ArtifactError::NotFound { id: id.clone() })?;
    let mut stmt = conn
        .prepare_cached(
            "SELECT meta FROM artifact_versions WHERE artifact_id = ?1 ORDER BY version",
        )
        .map_err(storage)?;
    let metas = stmt
        .query_map(params![id.0], |r| r.get::<_, String>(0))
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;
    let versions = metas
        .iter()
        .map(|m| serde_json::from_str::<ArtifactVersion>(m).map_err(storage))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Artifact {
        id: id.clone(),
        session: session.clone(),
        name,
        origin: serde_json::from_str(&origin).map_err(storage)?,
        versions,
    })
}

/// Identyfikatory w kolejności rejestracji.
pub fn ids(conn: &Connection) -> Result<Vec<ArtifactId>, ArtifactError> {
    let mut stmt = conn
        .prepare_cached("SELECT id FROM artifacts ORDER BY seq")
        .map_err(storage)?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(storage)?;
    rows.map(|r| r.map(ArtifactId).map_err(storage)).collect()
}

/// Dopisuje wersję, jeśli treść różni się od najnowszej; zwraca, czy dopisano.
pub fn push_version(
    conn: &Connection,
    artifact: &Artifact,
    path: &Path,
    facts: FileFacts,
    source_turn: Option<TurnId>,
) -> Result<bool, ArtifactError> {
    let Some(version) = next_version(&artifact.versions, &facts.sha256) else {
        return Ok(false);
    };
    let meta = ArtifactVersion {
        version,
        path: path.to_path_buf(),
        bytes: facts.bytes,
        sha256: facts.sha256,
        mime: facts.mime,
        source_turn,
        created_at: Utc::now(),
        snapshot: facts.snapshot.is_some(),
    };
    let json = serde_json::to_string(&meta).map_err(storage)?;
    conn.execute(
        "INSERT INTO artifact_versions(artifact_id, version, meta, snapshot) VALUES (?1, ?2, ?3, ?4)",
        params![artifact.id.0, version, json, facts.snapshot],
    )
    .map(|_| true)
    .map_err(storage)
}

/// Migawka treści wersji.
pub fn snapshot(
    conn: &Connection,
    id: &ArtifactId,
    version: u32,
) -> Result<Option<Vec<u8>>, ArtifactError> {
    conn.query_row(
        "SELECT snapshot FROM artifact_versions WHERE artifact_id = ?1 AND version = ?2",
        params![id.0, version],
        |r| r.get::<_, Option<Vec<u8>>>(0),
    )
    .optional()
    .map(Option::flatten)
    .map_err(storage)
}
