//! Artefakty sesji w paczkach `.alfa` (kategoria `artifacts`, „Eksport → artefakty”): magazyn
//! dokumentów nad rejestrem artefaktów. Nazwa dokumentu: `<sesja>/<artefakt>/<plik>` (najnowsza
//! wersja). Import zapisuje plik do `…\Sesje\Import\<sesja>\<artefakt>\<plik>` (nigdy poza tym
//! katalogiem; zapis atomowy) i rejestruje go w sesji, jeśli ta istnieje. Usuwanie (rollback
//! importu) — tylko plików z katalogu importu; pliki użytkownika poza nim nie są nigdy usuwane.
//!
//! Rejestr artefaktów powstaje w kompozycji później niż moduł `transfer` — stąd wiązanie
//! późne ([`ArtifactDocuments::bind`]); przed nim magazyn jest pusty (lista) albo niedostępny.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, OnceLock};

use artifacts_contract::{ArtifactId, Artifacts, Origin};
use sessions_contract::{SessionId, SessionQuery, Sessions};
use transfer_contract::{DocumentStore, TransferError, validate_entry_path};

/// Podkatalog importu w katalogu sesji użytkownika.
pub const IMPORT_DIR: &str = "Import";
/// Górna granica wpisów przeglądanych na poziom katalogu przy uzgadnianiu.
const MAX_SCAN: usize = 10_000;

struct Bound {
    artifacts: Arc<dyn Artifacts>,
    sessions: Arc<dyn Sessions>,
}

/// Artefakty sesji jako [`DocumentStore`].
pub struct ArtifactDocuments {
    import_root: PathBuf,
    bound: OnceLock<Bound>,
    max_bytes: u64,
}

fn port(e: impl std::fmt::Display) -> TransferError {
    TransferError::port("artifacts", e)
}

impl ArtifactDocuments {
    /// Magazyn z katalogiem importu (`%USERPROFILE%\Alfa\Sesje\Import`) i limitem pliku.
    pub fn new(import_root: PathBuf, max_bytes: u64) -> Arc<Self> {
        Arc::new(Self {
            import_root,
            bound: OnceLock::new(),
            max_bytes,
        })
    }

    /// Wiązanie z rejestrem artefaktów i sesjami (raz).
    pub fn bind(&self, artifacts: Arc<dyn Artifacts>, sessions: Arc<dyn Sessions>) {
        let _ = self.bound.set(Bound {
            artifacts,
            sessions,
        });
    }

    fn bound(&self) -> Result<&Bound, TransferError> {
        self.bound
            .get()
            .ok_or_else(|| port("rejestr artefaktów jeszcze niepodłączony"))
    }

    /// Rejestruje w sesjach pliki przywrócone z paczek, których sesja powstała dopiero po zapisie
    /// pliku (import zapisuje dokumenty przed sesjami). Zwraca liczbę nowych rejestracji.
    pub fn reconcile(&self) -> usize {
        let Ok(b) = self.bound() else {
            return 0;
        };
        let Ok(sessions) = std::fs::read_dir(&self.import_root) else {
            return 0;
        };
        let mut done = 0;
        for dir in sessions.flatten().take(MAX_SCAN) {
            let session = SessionId::new(dir.file_name().to_string_lossy());
            if b.sessions.session(&session).is_err() {
                continue;
            }
            let known: HashSet<PathBuf> = b
                .artifacts
                .list(&session)
                .map(|list| {
                    list.iter()
                        .flat_map(|a| a.versions.iter().map(|v| v.path.clone()))
                        .collect()
                })
                .unwrap_or_default();
            let files = std::fs::read_dir(dir.path())
                .into_iter()
                .flatten()
                .flatten()
                .take(MAX_SCAN)
                .flat_map(|a| std::fs::read_dir(a.path()).into_iter().flatten().flatten())
                .map(|f| f.path())
                .filter(|p| p.is_file() && !known.contains(p))
                .filter(|p| !p.to_string_lossy().ends_with(".tmp"));
            for path in files.take(MAX_SCAN) {
                let origin = Origin::Import {
                    source: "alfa".into(),
                };
                if b.artifacts.register(&session, &path, origin, None).is_ok() {
                    done += 1;
                }
            }
        }
        done
    }

    fn split(name: &str) -> Result<(SessionId, ArtifactId, &str), TransferError> {
        validate_entry_path(name).map_err(|reason| TransferError::UnsafePath {
            path: name.to_owned(),
            reason,
        })?;
        let mut parts = name.splitn(3, '/');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(s), Some(a), Some(f)) if !f.contains('/') => {
                Ok((SessionId::new(s), ArtifactId(a.to_owned()), f))
            }
            _ => Err(TransferError::invalid(
                name,
                "oczekiwano <sesja>/<artefakt>/<plik>",
            )),
        }
    }

    fn import_path(&self, name: &str) -> PathBuf {
        name.split('/')
            .fold(self.import_root.clone(), |p, s| p.join(s))
    }

    fn current(&self, b: &Bound, name: &str) -> Result<Option<PathBuf>, TransferError> {
        let (session, id, file) = Self::split(name)?;
        let Ok(artifact) = b.artifacts.get(&session, &id) else {
            return Ok(None);
        };
        let path = artifact.latest().map(|v| v.path.clone());
        Ok(path.filter(|p| p.file_name().is_some_and(|f| f.to_string_lossy() == file)))
    }
}

fn file_name_ok(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    validate_entry_path(&name).is_ok().then_some(name)
}

impl DocumentStore for ArtifactDocuments {
    fn list(&self) -> Result<Vec<String>, TransferError> {
        let Ok(b) = self.bound() else {
            return Ok(Vec::new());
        };
        self.reconcile();
        let query = SessionQuery {
            include_archived: true,
            ..SessionQuery::default()
        };
        let mut out = Vec::new();
        for summary in b.sessions.list_sessions(&query).map_err(port)? {
            let id = &summary.meta.id;
            for artifact in b.artifacts.list(id).map_err(port)? {
                let Some(latest) = artifact.latest() else {
                    continue;
                };
                let readable = std::fs::metadata(&latest.path)
                    .is_ok_and(|m| m.is_file() && m.len() <= self.max_bytes);
                if let (true, Some(file)) = (readable, file_name_ok(&latest.path)) {
                    let name = format!("{id}/{}/{file}", artifact.id);
                    if validate_entry_path(&name).is_ok() {
                        out.push(name);
                    }
                }
            }
        }
        out.sort();
        Ok(out)
    }

    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, TransferError> {
        let b = self.bound()?;
        let Some(path) = self.current(b, name)? else {
            return Ok(None);
        };
        match std::fs::metadata(&path) {
            Ok(m) if m.is_file() && m.len() <= self.max_bytes => {
                std::fs::read(&path).map(Some).map_err(port)
            }
            _ => Ok(None),
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), TransferError> {
        let b = self.bound()?;
        let (session, _, _) = Self::split(name)?;
        let path = self.import_path(name);
        let inside = path
            .strip_prefix(&self.import_root)
            .is_ok_and(|rel| rel.components().all(|c| matches!(c, Component::Normal(_))));
        if !inside {
            return Err(TransferError::invalid(
                name,
                "ścieżka poza katalogiem importu",
            ));
        }
        let dir = path.parent().unwrap_or(&self.import_root);
        std::fs::create_dir_all(dir).map_err(port)?;
        let tmp = dir.join(format!(".alfa-import-{}.tmp", std::process::id()));
        std::fs::write(&tmp, bytes).map_err(port)?;
        std::fs::rename(&tmp, &path).map_err(port)?;
        if b.sessions.session(&session).is_ok() {
            let origin = Origin::Import {
                source: "alfa".into(),
            };
            b.artifacts
                .register(&session, &path, origin, None)
                .map_err(port)?;
        }
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<bool, TransferError> {
        Self::split(name)?;
        let path = self.import_path(name);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(port(e)),
        }
    }
}
