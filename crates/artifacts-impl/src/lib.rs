//! Implementacja modułu `artifacts` (docs/modules/artifacts/SPEC.md, PLAN §11).
//!
//! Rejestr żyje w **szyfrowanej bazie sesji** (tabele `artifacts`, `artifact_versions`), więc
//! usunięcie sesji kasuje też metadane i migawki. Wersje są niezmienne (wyzwalacze). Treść wersji
//! ≤ limit migawki (domyślnie 1 MiB) jest zachowywana jako BLOB — podgląd i diff działają po
//! nadpisaniu pliku. Akcje UI zwracane są jako intencje dla `platform-windows`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod events;
mod store;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use artifacts_contract::{
    Artifact, ArtifactAction, ArtifactError, ArtifactId, ArtifactIntent, ArtifactVersion,
    Artifacts, BINARY_SNIFF_BYTES, DEFAULT_SNAPSHOT_MAX_BYTES, Origin, Preview, SessionId,
    TextDiff, TurnId, default_out_dir, diff_contents, events as names, preview_bytes,
    read_file_facts, validate_action, version_content,
};
use core_bus_contract::Level;
use core_registry_contract::{ManifestError, ModuleManifest};
use lib_sqlstore::Db;
use lib_sqlstore::rusqlite::Connection;
use serde_json::json;
use sessions_contract::SessionDbProvider;

use crate::events::Outbox;
use crate::store::storage;

pub use store::{MIGRATIONS, NAMESPACE};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Rejestr artefaktów na bazach sesji.
pub struct SqliteArtifacts {
    provider: Arc<dyn SessionDbProvider>,
    root: PathBuf,
    snapshot_max: u64,
    outbox: Outbox,
    manifest: ModuleManifest,
}

impl SqliteArtifacts {
    /// Rejestr; `root` = `%USERPROFILE%\Alfa` (katalogi sesji: `<root>\Sesje\<nazwa>\out`).
    pub fn new(provider: Arc<dyn SessionDbProvider>, root: PathBuf) -> Result<Self, ManifestError> {
        Ok(Self {
            provider,
            root,
            snapshot_max: DEFAULT_SNAPSHOT_MAX_BYTES,
            outbox: Outbox::default(),
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
        })
    }

    /// Limit migawki treści wersji (`[artifacts] snapshot_max_mb`).
    #[must_use]
    pub fn with_snapshot_max(mut self, bytes: u64) -> Self {
        self.snapshot_max = bytes;
        self
    }

    fn db(&self, session: &SessionId) -> Result<Arc<Db>, ArtifactError> {
        self.provider.session_db(session).map_err(storage)
    }

    fn with_conn<R>(
        &self,
        session: &SessionId,
        f: impl FnOnce(&mut Connection) -> Result<R, ArtifactError>,
    ) -> Result<R, ArtifactError> {
        self.db(session)?.with(|conn| {
            store::prepare(conn)?;
            f(conn)
        })
    }

    fn write_version(
        &self,
        session: &SessionId,
        target: Option<&ArtifactId>,
        path: &Path,
        origin: Origin,
        source_turn: Option<TurnId>,
    ) -> Result<Artifact, ArtifactError> {
        let facts = read_file_facts(path, self.snapshot_max)?;
        let (artifact, created, added) = self.with_conn(session, |conn| {
            let tx = conn.transaction().map_err(storage)?;
            let (id, created) = match target {
                Some(id) => (id.clone(), false),
                None => match store::find_by_path(&tx, path)? {
                    Some(id) => (id, false),
                    None => {
                        let id = ArtifactId(uuid::Uuid::now_v7().to_string());
                        store::insert_artifact(&tx, &id, &facts.name, path, &origin)?;
                        (id, true)
                    }
                },
            };
            let current = store::load(&tx, session, &id)?;
            let added = store::push_version(&tx, &current, path, facts, source_turn)?;
            let artifact = store::load(&tx, session, &id)?;
            tx.commit().map_err(storage)?;
            Ok((artifact, created, added))
        })?;
        if let Some(v) = artifact.latest().filter(|_| added) {
            let kind = if created {
                names::REGISTERED
            } else {
                names::VERSION_ADDED
            };
            let payload = json!({
                "artifact": artifact.id, "version": v.version, "mime": v.mime, "bytes": v.bytes,
            });
            self.outbox.emit(kind, Level::Info, Some(session), payload);
        }
        Ok(artifact)
    }

    fn content(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        version: Option<u32>,
        limit: Option<usize>,
    ) -> Result<(ArtifactVersion, Vec<u8>), ArtifactError> {
        let (v, latest, snapshot) = self.with_conn(session, |conn| {
            let artifact = store::load(conn, session, id)?;
            let latest = artifact.latest().map(|v| v.version).unwrap_or_default();
            let wanted = version.unwrap_or(latest);
            let v = artifact.version(wanted).cloned().ok_or_else(|| {
                ArtifactError::VersionNotFound {
                    id: id.clone(),
                    version: wanted,
                }
            })?;
            let snapshot = store::snapshot(conn, id, wanted)?;
            Ok((v, wanted == latest, snapshot))
        })?;
        let bytes = version_content(&v, snapshot, latest, limit)?;
        Ok((v, bytes))
    }
}

impl Artifacts for SqliteArtifacts {
    fn out_dir(&self, dir_name: &str) -> PathBuf {
        default_out_dir(&self.root, dir_name)
    }

    fn register(
        &self,
        session: &SessionId,
        path: &Path,
        origin: Origin,
        source_turn: Option<TurnId>,
    ) -> Result<Artifact, ArtifactError> {
        self.write_version(session, None, path, origin, source_turn)
    }

    fn add_version(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        path: &Path,
        source_turn: Option<TurnId>,
    ) -> Result<Artifact, ArtifactError> {
        let origin = self.get(session, id)?.origin;
        self.write_version(session, Some(id), path, origin, source_turn)
    }

    fn get(&self, session: &SessionId, id: &ArtifactId) -> Result<Artifact, ArtifactError> {
        self.with_conn(session, |conn| store::load(conn, session, id))
    }

    fn list(&self, session: &SessionId) -> Result<Vec<Artifact>, ArtifactError> {
        self.with_conn(session, |conn| {
            store::ids(conn)?
                .iter()
                .map(|id| store::load(conn, session, id))
                .collect()
        })
    }

    fn preview(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        version: Option<u32>,
        max_bytes: usize,
    ) -> Result<Preview, ArtifactError> {
        let limit = max_bytes.max(BINARY_SNIFF_BYTES);
        let (v, bytes) = self.content(session, id, version, Some(limit))?;
        Ok(preview_bytes(&bytes, v.bytes, max_bytes, &v.mime))
    }

    fn diff(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        from: u32,
        to: u32,
    ) -> Result<TextDiff, ArtifactError> {
        let (_, old) = self.content(session, id, Some(from), None)?;
        let (_, new) = self.content(session, id, Some(to), None)?;
        diff_contents(&old, &new)
    }

    fn intent(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        version: Option<u32>,
        action: ArtifactAction,
    ) -> Result<ArtifactIntent, ArtifactError> {
        validate_action(session, &action)?;
        let artifact = self.get(session, id)?;
        let wanted =
            version.unwrap_or_else(|| artifact.latest().map(|v| v.version).unwrap_or_default());
        let v = artifact
            .version(wanted)
            .ok_or_else(|| ArtifactError::VersionNotFound {
                id: id.clone(),
                version: wanted,
            })?;
        let kind = match action {
            ArtifactAction::SendToSession { .. } => names::HANDOFF,
            _ => names::EXPORTED,
        };
        let intent = ArtifactIntent {
            session: session.clone(),
            artifact: id.clone(),
            version: v.version,
            path: v.path.clone(),
            sha256: v.sha256.clone(),
            action,
        };
        let payload = json!({ "artifact": id, "version": v.version, "action": intent.action });
        self.outbox.emit(kind, Level::Info, Some(session), payload);
        Ok(intent)
    }
}
