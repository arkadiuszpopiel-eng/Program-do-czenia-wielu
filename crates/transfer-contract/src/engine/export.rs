//! Eksport: dokumenty kategorii, sesje (format przenośny), sekrety (tylko paczka `secrets`),
//! snapshot przed importem. Strażnik sekretów dla zwykłych paczek (`export`, `backup`).

use std::collections::BTreeSet;

use accounts_hub_contract::SecretString;
use sessions_contract::{PrivacyTag, SessionId, SessionQuery};

use crate::engine::{Engine, ImportPlan, RollbackData, SecretRecord};
use crate::error::TransferError;
use crate::guard::SecretGuard;
use crate::manifest::{
    ContentEntry, EncryptionInfo, Manifest, PackageKind, ScopeSummary, content_sha256,
    schema_version,
};
use crate::paths::{
    ROLLBACK_PATH, SECRETS_PATH, SESSION_FILE, TURNS_FILE, document_path, session_path,
    validate_entry_path,
};
use crate::portable::encode_session;
use crate::ports::PackageSink;
use crate::report::{ItemRef, Warning};
use crate::scope::{CancelToken, Category, ExportScope, Selection};

/// Parametry eksportu dla silnika.
#[derive(Debug, Clone, Copy)]
pub struct ExportSpec<'a> {
    /// Rodzaj paczki.
    pub kind: PackageKind,
    /// Zakres.
    pub scope: &'a ExportScope,
    /// Szyfrowanie paczki (wymagane dla sesji prywatnych i sekretów).
    pub encryption: Option<&'a EncryptionInfo>,
    /// Opis.
    pub notes: Option<&'a str>,
    /// Anulowanie.
    pub cancel: Option<&'a CancelToken>,
}

/// Wynik eksportu silnika (kontener dopisuje manifest i szyfruje).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOutcome {
    /// Manifest.
    pub manifest: Manifest,
    /// Ostrzeżenia.
    pub warnings: Vec<Warning>,
}

/// Zbieracz wpisów: strażnik, lista zawartości, liczniki.
struct Writer<'s> {
    sink: &'s mut dyn PackageSink,
    guard: Option<SecretGuard>,
    content: Vec<ContentEntry>,
    scope: ScopeSummary,
    keys: BTreeSet<&'static str>,
    warnings: Vec<Warning>,
    redactions: u64,
    /// Zerowanie buforów po zapisie (paczki z sekretami: `secrets`, `snapshot`).
    zeroize: bool,
}

impl Writer<'_> {
    fn add(&mut self, path: &str, bytes: Vec<u8>) -> Result<(), TransferError> {
        validate_entry_path(path).map_err(|reason| TransferError::UnsafePath {
            path: path.to_owned(),
            reason,
        })?;
        let bytes = match &self.guard {
            Some(guard) => {
                let (clean, n) = guard.clean_document(path, bytes)?;
                if n > 0 {
                    self.redactions += n;
                    self.warnings.push(Warning::Redacted {
                        path: path.to_owned(),
                        count: n,
                    });
                }
                if guard.find_leak(&clean) {
                    return Err(TransferError::SecretDetected {
                        path: path.to_owned(),
                    });
                }
                clean
            }
            None => bytes,
        };
        let mut bytes = bytes;
        let added = self.sink.add(path, &bytes);
        if added.is_ok() {
            self.content.push(ContentEntry::of(path, &bytes));
        }
        if self.zeroize {
            use zeroize::Zeroize;
            bytes.zeroize();
        }
        added
    }
}

impl Engine<'_> {
    fn writer<'s>(&self, sink: &'s mut dyn PackageSink, kind: PackageKind) -> Writer<'s> {
        let mut warnings = Vec::new();
        let guard = match kind {
            PackageKind::Export | PackageKind::Backup => {
                let values: Vec<SecretString> = match self.read_secrets() {
                    Ok(list) => list.into_iter().map(|(_, v)| v).collect(),
                    Err(_) => {
                        warnings.push(Warning::SecretStoreUnavailable);
                        Vec::new()
                    }
                };
                if self.ports().secrets.is_none() {
                    warnings.push(Warning::SecretStoreUnavailable);
                }
                Some(SecretGuard::new(values))
            }
            PackageKind::Secrets | PackageKind::Snapshot => None,
        };
        Writer {
            sink,
            guard,
            content: Vec::new(),
            scope: ScopeSummary::default(),
            keys: BTreeSet::new(),
            warnings,
            redactions: 0,
            zeroize: matches!(kind, PackageKind::Secrets | PackageKind::Snapshot),
        }
    }

    fn finish(
        &self,
        w: Writer<'_>,
        kind: PackageKind,
        spec_notes: Option<&str>,
        encryption: Option<&EncryptionInfo>,
    ) -> ExportOutcome {
        let mut scope = w.scope;
        scope.keys = w.keys.iter().map(|k| (*k).to_owned()).collect();
        let manifest = Manifest {
            schema_version: schema_version(),
            app_version: self.ports().app_version.clone(),
            kind,
            created_at: self.ports().clock.now(),
            source_machine: self.ports().machine.clone(),
            scope,
            content_sha256: content_sha256(&w.content),
            content: w.content,
            encryption: encryption.cloned(),
            notes: spec_notes.map(str::to_owned),
            redactions: w.redactions,
        };
        ExportOutcome {
            manifest,
            warnings: w.warnings,
        }
    }

    /// Eksport zakresu (`export`/`backup`).
    pub fn export(
        &self,
        spec: &ExportSpec<'_>,
        sink: &mut dyn PackageSink,
    ) -> Result<ExportOutcome, TransferError> {
        let mut w = self.writer(sink, spec.kind);
        let sessions = self.selected_sessions(spec, &mut w.warnings)?;
        for category in Category::DOCUMENTS {
            if !spec.scope.includes(category) {
                continue;
            }
            let Some(store) = self.ports().store(category) else {
                w.warnings.push(Warning::CategoryUnavailable { category });
                continue;
            };
            for name in store.list()? {
                CancelToken::check(spec.cancel)?;
                if !self.wanted(category, &name, spec.scope, &sessions) {
                    continue;
                }
                let Some(bytes) = store.read(&name)? else {
                    continue;
                };
                if category == Category::Memory {
                    w.scope.counts.memory_entries += bytes
                        .split(|b| *b == b'\n')
                        .filter(|l| !l.is_empty())
                        .count() as u64;
                }
                if category == Category::Artifacts {
                    w.scope.counts.artifacts += 1;
                }
                w.add(&document_path(category, &name), bytes)?;
                w.scope.counts.documents += 1;
                w.keys.insert(category.key());
            }
        }
        for id in &sessions {
            CancelToken::check(spec.cancel)?;
            self.export_session(&mut w, id)?;
        }
        Ok(self.finish(w, spec.kind, spec.notes, spec.encryption))
    }

    fn export_session(&self, w: &mut Writer<'_>, id: &SessionId) -> Result<(), TransferError> {
        let session = self
            .local_session(id)?
            .ok_or_else(|| TransferError::NotFound {
                what: format!("sesja {id}"),
            })?;
        let (header, turns) = encode_session(&session, self.ports().workdir_root.as_deref())?;
        w.add(&session_path(id, TURNS_FILE), turns)?;
        w.add(&session_path(id, SESSION_FILE), header)?;
        w.scope.sessions.push(id.clone());
        w.scope.counts.sessions += 1;
        w.scope.counts.turns += session.turns.len() as u64;
        w.keys.insert(Category::Sessions.key());
        Ok(())
    }

    fn wanted(
        &self,
        category: Category,
        name: &str,
        scope: &ExportScope,
        sessions: &[SessionId],
    ) -> bool {
        match category {
            Category::Memory => match &scope.memory {
                Selection::All => true,
                Selection::None => false,
                Selection::Only(sel) => sel.iter().any(|s| {
                    name == s
                        || name.starts_with(&format!("{s}/"))
                        || name.rsplit_once('.').is_some_and(|(stem, _)| stem == s)
                }),
            },
            Category::Artifacts => sessions
                .iter()
                .any(|id| name.starts_with(&format!("{id}/"))),
            Category::ConfigMachine => {
                let id = &self.ports().machine.id;
                id.is_empty() || name == format!("{id}.toml")
            }
            _ => true,
        }
    }

    /// Sesje do eksportu: wybór, bez kosza; prywatne tylko jawnie i w paczce szyfrowanej.
    fn selected_sessions(
        &self,
        spec: &ExportSpec<'_>,
        warnings: &mut Vec<Warning>,
    ) -> Result<Vec<SessionId>, TransferError> {
        let ids: Vec<SessionId> = match &spec.scope.sessions {
            Selection::None => return Ok(Vec::new()),
            Selection::All => {
                let query = SessionQuery {
                    include_archived: true,
                    ..SessionQuery::default()
                };
                self.ports()
                    .sessions()?
                    .list_sessions(&query)?
                    .into_iter()
                    .map(|s| s.meta.id)
                    .collect()
            }
            Selection::Only(ids) => ids.clone(),
        };
        let mut out = Vec::new();
        for id in ids {
            let meta = self.ports().sessions()?.session(&id)?;
            if meta.privacy != PrivacyTag::Normal && spec.kind != PackageKind::Snapshot {
                if !spec.scope.include_private {
                    warnings.push(Warning::PrivateSession { id, skipped: true });
                    continue;
                }
                if spec.encryption.is_none() {
                    return Err(TransferError::EncryptionRequired {
                        what: format!("eksport prywatnej sesji {id}"),
                    });
                }
            }
            if !out.contains(&id) {
                out.push(id);
            }
        }
        out.sort();
        Ok(out)
    }

    /// Eksport sekretów (paczka `secrets`; kontener wymusza szyfrowanie hasłem).
    pub fn export_secrets(
        &self,
        sink: &mut dyn PackageSink,
        encryption: &EncryptionInfo,
    ) -> Result<ExportOutcome, TransferError> {
        let mut w = self.writer(sink, PackageKind::Secrets);
        let secrets = self.read_secrets()?;
        let records: Vec<SecretRecord> = secrets
            .iter()
            .map(|(n, v)| SecretRecord {
                name: n.as_str().to_owned(),
                value: v.expose_secret().to_owned(),
            })
            .collect();
        let bytes =
            serde_json::to_vec(&records).map_err(|e| TransferError::invalid(SECRETS_PATH, e))?;
        drop(records);
        w.add(SECRETS_PATH, bytes)?;
        w.scope.counts.secrets = secrets.len() as u64;
        w.keys.insert(Category::Secrets.key());
        Ok(self.finish(w, PackageKind::Secrets, None, Some(encryption)))
    }

    /// Snapshot elementów, które import zmieni (stan sprzed importu) + `rollback.json`.
    /// Sekrety trafiają do snapshotu tylko, gdy `secrets_allowed` (snapshot szyfrowany).
    pub fn snapshot(
        &self,
        plan: &ImportPlan,
        sink: &mut dyn PackageSink,
        encryption: Option<&EncryptionInfo>,
    ) -> Result<ExportOutcome, TransferError> {
        let mut w = self.writer(sink, PackageKind::Snapshot);
        let mut data = RollbackData {
            v: 1,
            ..RollbackData::default()
        };
        let mut secrets = Vec::new();
        for step in plan.steps() {
            if !step.action.writes() {
                continue;
            }
            match &step.target {
                ItemRef::Document { category, name } => {
                    let local = match self.ports().store(*category) {
                        Some(store) => store.read(name)?,
                        None => None,
                    };
                    match local {
                        Some(bytes) => {
                            w.add(&document_path(*category, name), bytes)?;
                            data.saved.push(step.target.clone());
                        }
                        None => data.created.push(step.target.clone()),
                    }
                }
                ItemRef::Session { id } => match self.local_session(id)? {
                    Some(_) => {
                        self.export_session(&mut w, id)?;
                        data.saved.push(step.target.clone());
                    }
                    None => data.created.push(step.target.clone()),
                },
                ItemRef::Secret { name } => {
                    let current = self
                        .read_secrets()?
                        .into_iter()
                        .find(|(n, _)| n.as_str() == name);
                    match current {
                        Some((n, v)) => {
                            secrets.push(SecretRecord {
                                name: n.as_str().to_owned(),
                                value: v.expose_secret().to_owned(),
                            });
                            data.saved.push(step.target.clone());
                        }
                        None => data.created.push(step.target.clone()),
                    }
                }
            }
        }
        if !secrets.is_empty() {
            if encryption.is_none() {
                return Err(TransferError::EncryptionRequired {
                    what: "snapshot sekretów".to_owned(),
                });
            }
            let bytes = serde_json::to_vec(&secrets)
                .map_err(|e| TransferError::invalid(SECRETS_PATH, e))?;
            drop(secrets);
            w.add(SECRETS_PATH, bytes)?;
        }
        let rollback = serde_json::to_vec_pretty(&data)
            .map_err(|e| TransferError::invalid(ROLLBACK_PATH, e))?;
        w.add(ROLLBACK_PATH, rollback)?;
        Ok(self.finish(w, PackageKind::Snapshot, None, encryption))
    }
}
