//! Atrapa modułu `artifacts` (docs/modules/artifacts/SPEC.md, „Fake”).
//!
//! Rejestr w pamięci: pliki czytane z dysku (w testach — katalog tymczasowy), migawki treści w RAM,
//! deterministyczne identyfikatory (`art-0001`…) i wirtualny zegar. Zamiast Eksploratora —
//! rejestr utworzonych intencji ([`FakeArtifacts::intents`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use artifacts_contract::{
    Artifact, ArtifactAction, ArtifactError, ArtifactId, ArtifactIntent, ArtifactVersion,
    Artifacts, BINARY_SNIFF_BYTES, DEFAULT_SNAPSHOT_MAX_BYTES, FileFacts, Origin, Preview,
    SessionId, TextDiff, TurnId, default_out_dir, diff_contents, next_version, preview_bytes,
    read_file_facts, validate_action, version_content,
};
use chrono::{DateTime, TimeZone, Utc};

#[derive(Debug, Clone)]
struct Stored {
    artifact: Artifact,
    snapshots: BTreeMap<u32, Vec<u8>>,
}

#[derive(Debug, Default)]
struct State {
    sessions: BTreeMap<SessionId, Vec<Stored>>,
    intents: Vec<ArtifactIntent>,
    next_id: u64,
    ticks: i64,
}

impl State {
    fn now(&mut self) -> DateTime<Utc> {
        self.ticks += 1;
        let base = Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .single()
            .unwrap_or_default();
        base + chrono::Duration::seconds(self.ticks)
    }

    fn find(&self, session: &SessionId, id: &ArtifactId) -> Result<&Stored, ArtifactError> {
        self.sessions
            .get(session)
            .and_then(|v| v.iter().find(|s| s.artifact.id == *id))
            .ok_or_else(|| ArtifactError::NotFound { id: id.clone() })
    }
}

/// Rejestr artefaktów w pamięci.
#[derive(Debug)]
pub struct FakeArtifacts {
    state: Mutex<State>,
    root: PathBuf,
    snapshot_max: u64,
}

impl Default for FakeArtifacts {
    fn default() -> Self {
        Self::new(PathBuf::from("C:\\Users\\atrapa\\Alfa"))
    }
}

impl FakeArtifacts {
    /// Rejestr z korzeniem katalogów sesji `root` i limitem migawki 1 MiB.
    pub fn new(root: PathBuf) -> Self {
        Self {
            state: Mutex::new(State::default()),
            root,
            snapshot_max: DEFAULT_SNAPSHOT_MAX_BYTES,
        }
    }

    /// Zmienia limit migawki treści (testy dużych plików).
    #[must_use]
    pub fn with_snapshot_max(mut self, bytes: u64) -> Self {
        self.snapshot_max = bytes;
        self
    }

    /// Intencje utworzone przez `intent` (w kolejności).
    pub fn intents(&self) -> Vec<ArtifactIntent> {
        self.lock().intents.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push_version(
        st: &mut State,
        session: &SessionId,
        id: &ArtifactId,
        path: &Path,
        facts: FileFacts,
        source_turn: Option<TurnId>,
    ) -> Result<Artifact, ArtifactError> {
        let now = st.now();
        let stored = st
            .sessions
            .get_mut(session)
            .and_then(|v| v.iter_mut().find(|s| s.artifact.id == *id))
            .ok_or_else(|| ArtifactError::NotFound { id: id.clone() })?;
        if let Some(version) = next_version(&stored.artifact.versions, &facts.sha256) {
            stored.artifact.versions.push(ArtifactVersion {
                version,
                path: path.to_path_buf(),
                bytes: facts.bytes,
                sha256: facts.sha256,
                mime: facts.mime,
                source_turn,
                created_at: now,
                snapshot: facts.snapshot.is_some(),
            });
            if let Some(bytes) = facts.snapshot {
                stored.snapshots.insert(version, bytes);
            }
        }
        Ok(stored.artifact.clone())
    }

    fn content(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        version: Option<u32>,
        limit: Option<usize>,
    ) -> Result<(ArtifactVersion, Vec<u8>), ArtifactError> {
        let st = self.lock();
        let stored = st.find(session, id)?;
        let latest = stored
            .artifact
            .latest()
            .map(|v| v.version)
            .unwrap_or_default();
        let wanted = version.unwrap_or(latest);
        let v = stored.artifact.version(wanted).cloned().ok_or_else(|| {
            ArtifactError::VersionNotFound {
                id: id.clone(),
                version: wanted,
            }
        })?;
        let snapshot = stored.snapshots.get(&wanted).cloned();
        let bytes = version_content(&v, snapshot, wanted == latest, limit)?;
        Ok((v, bytes))
    }
}

impl Artifacts for FakeArtifacts {
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
        let facts = read_file_facts(path, self.snapshot_max)?;
        let mut st = self.lock();
        let existing = st.sessions.get(session).and_then(|v| {
            v.iter()
                .find(|s| s.artifact.versions.first().is_some_and(|f| f.path == path))
                .map(|s| s.artifact.id.clone())
        });
        let id = match existing {
            Some(id) => id,
            None => {
                st.next_id += 1;
                let id = ArtifactId(format!("art-{:04}", st.next_id));
                let artifact = Artifact {
                    id: id.clone(),
                    session: session.clone(),
                    name: facts.name.clone(),
                    origin,
                    versions: Vec::new(),
                };
                st.sessions
                    .entry(session.clone())
                    .or_default()
                    .push(Stored {
                        artifact,
                        snapshots: BTreeMap::new(),
                    });
                id
            }
        };
        Self::push_version(&mut st, session, &id, path, facts, source_turn)
    }

    fn add_version(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        path: &Path,
        source_turn: Option<TurnId>,
    ) -> Result<Artifact, ArtifactError> {
        self.lock().find(session, id)?;
        let facts = read_file_facts(path, self.snapshot_max)?;
        Self::push_version(&mut self.lock(), session, id, path, facts, source_turn)
    }

    fn get(&self, session: &SessionId, id: &ArtifactId) -> Result<Artifact, ArtifactError> {
        Ok(self.lock().find(session, id)?.artifact.clone())
    }

    fn list(&self, session: &SessionId) -> Result<Vec<Artifact>, ArtifactError> {
        Ok(self
            .lock()
            .sessions
            .get(session)
            .map(|v| v.iter().map(|s| s.artifact.clone()).collect())
            .unwrap_or_default())
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
        let mut st = self.lock();
        let artifact = &st.find(session, id)?.artifact;
        let latest = artifact.latest().map(|v| v.version).unwrap_or_default();
        let wanted = version.unwrap_or(latest);
        let v = artifact
            .version(wanted)
            .ok_or_else(|| ArtifactError::VersionNotFound {
                id: id.clone(),
                version: wanted,
            })?;
        let intent = ArtifactIntent {
            session: session.clone(),
            artifact: id.clone(),
            version: v.version,
            path: v.path.clone(),
            sha256: v.sha256.clone(),
            action,
        };
        st.intents.push(intent.clone());
        Ok(intent)
    }
}
