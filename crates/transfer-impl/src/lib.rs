//! Implementacja modułu `transfer` (docs/modules/transfer/SPEC.md, docs/formats/alfa-package.md).
//!
//! [`ZipTransfer`] = silnik z `transfer-contract` + kontener ZIP ([`container`]) + szyfrowanie
//! ([`crypto`]: Argon2id + XChaCha20-Poly1305 STREAM) + snapshoty przed importem (szyfrowane
//! kluczem maszyny z Credential Managera, rotacja) + kopie zapasowe z rotacją. Magazyn dokumentów
//! na katalogu: [`DirDocumentStore`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod container;
pub mod crypto;
mod dirstore;
mod events;
mod files;
mod tempfile;

use std::path::{Path, PathBuf};

use accounts_hub_contract::{SecretName, SecretString};
use core_registry_contract::{ManifestError, ModuleManifest};
use sessions_contract::SessionId;
use transfer_contract::backup::{
    BACKUP_PREFIX, DEFAULT_SNAPSHOTS_KEEP, EXTENSION, SNAPSHOT_PREFIX, parse_stamp,
};
use transfer_contract::engine::{Engine, ExportSpec, SNAPSHOT_KEY_SECRET};
use transfer_contract::{
    BackupReport, BackupRequest, CancelToken, ExportReport, ExportRequest, IdSource, ImportOptions,
    ImportReport, Inspection, PackageKind, RollbackReport, SnapshotId, SnapshotInfo, Transfer,
    TransferError, TransferPorts, Warning, events as ev, validate_password,
};
use zeroize::Zeroizing;

pub use crypto::KdfParams;
pub use dirstore::{DirDocumentStore, DirFilter};
pub use events::Outbox;

use crate::container::{ZipSource, open_package, write_package};
use crate::crypto::Sealer;
use crate::files::{rotate, unique_path};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Konfiguracja (`[transfer]` w TOML; ścieżki rozwinięte przez wywołującego).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferConfig {
    /// Katalog snapshotów (`%LOCALAPPDATA%\Alfa\snapshots`).
    pub snapshots_dir: PathBuf,
    /// Liczba zachowanych snapshotów.
    pub snapshots_keep: usize,
    /// Parametry Argon2id dla nowych paczek.
    pub kdf: KdfParams,
}

impl TransferConfig {
    /// Konfiguracja domyślna w katalogu snapshotów.
    pub fn new(snapshots_dir: impl Into<PathBuf>) -> Self {
        Self {
            snapshots_dir: snapshots_dir.into(),
            snapshots_keep: DEFAULT_SNAPSHOTS_KEEP,
            kdf: KdfParams::default(),
        }
    }
}

/// Identyfikatory kopii sesji: UUIDv7 (jak `sessions-impl`).
#[derive(Debug, Default, Clone, Copy)]
pub struct UuidIds;

impl IdSource for UuidIds {
    fn new_session_id(&self) -> SessionId {
        SessionId::new(uuid::Uuid::now_v7().to_string())
    }
}

/// Moduł `transfer` na plikach `.alfa`.
pub struct ZipTransfer {
    ports: TransferPorts,
    config: TransferConfig,
    outbox: Outbox,
    manifest: ModuleManifest,
}

fn manifest_err(e: ManifestError) -> TransferError {
    TransferError::invalid("module.toml", e)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl ZipTransfer {
    /// Moduł na portach; sprząta pliki tymczasowe po przerwanym zapisie snapshotu.
    pub fn new(ports: TransferPorts, config: TransferConfig) -> Result<Self, TransferError> {
        let manifest = ModuleManifest::parse_toml(MODULE_TOML).map_err(manifest_err)?;
        tempfile::sweep(&config.snapshots_dir);
        Ok(Self {
            ports,
            config,
            outbox: Outbox::default(),
            manifest,
        })
    }

    /// Porty.
    pub fn ports(&self) -> &TransferPorts {
        &self.ports
    }

    /// Klucz maszyny do snapshotów (Credential Manager); `create` — wygeneruj, gdy brak.
    fn machine_key(&self, create: bool) -> Result<Option<Zeroizing<[u8; 32]>>, TransferError> {
        let Some(store) = &self.ports.secrets else {
            return Ok(None);
        };
        let name = SecretName::new(SNAPSHOT_KEY_SECRET)
            .map_err(|e| TransferError::invalid(SNAPSHOT_KEY_SECRET, e))?;
        if let Some(existing) = store.get(&name)? {
            return Ok(crypto::parse_machine_key(&existing));
        }
        if !create {
            return Ok(None);
        }
        let key = crypto::new_machine_key()?;
        store.put(&name, &key)?;
        Ok(crypto::parse_machine_key(&key))
    }

    fn open(
        &self,
        path: &Path,
        password: Option<&SecretString>,
    ) -> Result<ZipSource, TransferError> {
        let machine = |name: &str| {
            (name == SNAPSHOT_KEY_SECRET)
                .then(|| self.machine_key(false).ok().flatten())
                .flatten()
        };
        open_package(path, &self.ports.limits, &|header| {
            crypto::key_for(header, password, &machine)
        })
    }

    fn sealer(
        &self,
        kind: PackageKind,
        password: Option<&SecretString>,
    ) -> Result<Option<Sealer>, TransferError> {
        match password {
            Some(p) => {
                validate_password(p)?;
                Ok(Some(Sealer::with_password(kind, p, self.config.kdf)?))
            }
            None => Ok(None),
        }
    }

    fn take_snapshot(
        &self,
        engine: &Engine<'_>,
        plan: &transfer_contract::engine::ImportPlan,
        warnings: &mut Vec<Warning>,
    ) -> Result<SnapshotId, TransferError> {
        let sealer = match self.machine_key(true) {
            Ok(Some(key)) => Some(Sealer::with_machine_key(
                PackageKind::Snapshot,
                SNAPSHOT_KEY_SECRET,
                &key,
            )?),
            _ => {
                warnings.push(Warning::SnapshotUnencrypted);
                None
            }
        };
        let (id, path) = unique_path(
            &self.config.snapshots_dir,
            SNAPSHOT_PREFIX,
            self.ports.clock.now(),
        );
        write_package(&path, sealer.as_ref(), |sink| {
            engine
                .snapshot(plan, sink, sealer.as_ref().map(Sealer::info))
                .map(|o| o.manifest)
        })?;
        rotate(
            &self.config.snapshots_dir,
            SNAPSHOT_PREFIX,
            self.config.snapshots_keep.max(1),
        );
        self.outbox.emit(
            ev::IMPORT_SNAPSHOT_CREATED,
            serde_json::json!({ "snapshot": id, "items": plan.report.writes() }),
        );
        Ok(id)
    }

    fn snapshot_path(&self, id: &SnapshotId) -> Result<PathBuf, TransferError> {
        let valid = id.starts_with(SNAPSHOT_PREFIX)
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        let path = self.config.snapshots_dir.join(format!("{id}.{EXTENSION}"));
        if !valid || !path.is_file() {
            return Err(TransferError::NotFound {
                what: format!("snapshot {id}"),
            });
        }
        Ok(path)
    }
}

impl Transfer for ZipTransfer {
    fn export(&self, request: &ExportRequest) -> Result<ExportReport, TransferError> {
        if !matches!(request.kind, PackageKind::Export | PackageKind::Backup) {
            return Err(TransferError::invalid(
                "kind",
                "eksport obsługuje tylko export/backup (sekrety: export_secrets)",
            ));
        }
        CancelToken::check(request.cancel.as_ref())?;
        let sealer = self.sealer(request.kind, request.password.as_ref())?;
        self.outbox.emit(
            ev::EXPORT_STARTED,
            serde_json::json!({ "kind": request.kind.as_str(), "file": file_name(&request.dest) }),
        );
        let engine = Engine::new(&self.ports);
        let mut warnings = Vec::new();
        let (manifest, size) = write_package(&request.dest, sealer.as_ref(), |sink| {
            let spec = ExportSpec {
                kind: request.kind,
                scope: &request.scope,
                encryption: sealer.as_ref().map(Sealer::info),
                notes: request.notes.as_deref(),
                cancel: request.cancel.as_ref(),
            };
            let outcome = engine.export(&spec, sink)?;
            warnings = outcome.warnings;
            Ok(outcome.manifest)
        })?;
        self.outbox.emit(
            ev::EXPORT_COMPLETED,
            serde_json::json!({ "kind": request.kind.as_str(), "file": file_name(&request.dest), "entries": manifest.content.len(), "bytes": size, "redactions": manifest.redactions }),
        );
        Ok(ExportReport {
            path: request.dest.clone(),
            file_bytes: size,
            manifest,
            warnings,
        })
    }

    fn export_secrets(
        &self,
        dest: &Path,
        password: &SecretString,
    ) -> Result<ExportReport, TransferError> {
        let sealer = self
            .sealer(PackageKind::Secrets, Some(password))?
            .ok_or_else(|| TransferError::EncryptionRequired {
                what: "eksport sekretów".to_owned(),
            })?;
        let engine = Engine::new(&self.ports);
        let mut warnings = Vec::new();
        let (manifest, size) = write_package(dest, Some(&sealer), |sink| {
            let outcome = engine.export_secrets(sink, sealer.info())?;
            warnings = outcome.warnings;
            Ok(outcome.manifest)
        })?;
        self.outbox.emit(ev::EXPORT_COMPLETED, serde_json::json!({ "kind": "secrets", "file": file_name(dest), "secrets": manifest.scope.counts.secrets }));
        Ok(ExportReport {
            path: dest.to_path_buf(),
            file_bytes: size,
            manifest,
            warnings,
        })
    }

    fn inspect(
        &self,
        package: &Path,
        options: &ImportOptions,
    ) -> Result<Inspection, TransferError> {
        let mut source = self.open(package, options.password.as_ref())?;
        let plan = Engine::new(&self.ports).plan(&mut source, options)?;
        let payload = serde_json::json!({ "items": plan.report.items.len(), "writes": plan.report.writes(), "collisions": plan.report.collisions().len() });
        self.outbox.emit(ev::IMPORT_DRY_RUN, payload);
        Ok(Inspection {
            manifest: source.manifest_owned(),
            report: plan.report,
        })
    }

    fn import(
        &self,
        package: &Path,
        options: &ImportOptions,
    ) -> Result<ImportReport, TransferError> {
        let mut source = self.open(package, options.password.as_ref())?;
        let engine = Engine::new(&self.ports);
        let plan = engine.plan(&mut source, options)?;
        let mut extra = Vec::new();
        let snapshot = match plan.report.writes() {
            0 => None,
            _ => Some(self.take_snapshot(&engine, &plan, &mut extra)?),
        };
        let mut report = engine.apply(&mut source, &plan, options.cancel.as_ref())?;
        report.snapshot = snapshot;
        report.warnings.extend(extra);
        self.outbox.emit(
            ev::IMPORT_COMPLETED,
            serde_json::json!({ "snapshot": report.snapshot, "added": report.added, "merged": report.merged, "replaced": report.replaced, "copied": report.copied, "failed": report.failed }),
        );
        Ok(report)
    }

    fn snapshots(&self) -> Result<Vec<SnapshotInfo>, TransferError> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.config.snapshots_dir) else {
            return Ok(out);
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(&format!(".{EXTENSION}")) else {
                continue;
            };
            let Some(created_at) = parse_stamp(SNAPSHOT_PREFIX, stem) else {
                continue;
            };
            let items = self
                .open(&entry.path(), None)
                .map_or(0, |s| s.manifest_owned().content.len() as u64);
            out.push(SnapshotInfo {
                id: stem.to_owned(),
                created_at,
                items,
            });
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    fn rollback(&self, snapshot: &SnapshotId) -> Result<RollbackReport, TransferError> {
        let path = self.snapshot_path(snapshot)?;
        let mut source = self.open(&path, None)?;
        let report = Engine::new(&self.ports).rollback(&mut source)?;
        self.outbox.emit(ev::ROLLED_BACK, serde_json::json!({ "snapshot": snapshot, "restored": report.restored, "removed": report.removed }));
        Ok(report)
    }

    fn backup(&self, request: &BackupRequest) -> Result<BackupReport, TransferError> {
        std::fs::create_dir_all(&request.dir)?;
        let (_, dest) = unique_path(&request.dir, BACKUP_PREFIX, self.ports.clock.now());
        let export = self.export(&ExportRequest {
            scope: request.scope.clone(),
            dest,
            kind: PackageKind::Backup,
            password: request.password.clone(),
            notes: None,
            cancel: request.cancel.clone(),
        })?;
        let rotated_out = rotate(&request.dir, BACKUP_PREFIX, request.keep.max(1));
        self.outbox.emit(ev::BACKUP_COMPLETED, serde_json::json!({ "file": file_name(&export.path), "rotated_out": rotated_out.len() }));
        Ok(BackupReport {
            export,
            rotated_out,
        })
    }
}
