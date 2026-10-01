//! Atrapa modułu `transfer` (docs/modules/transfer/SPEC.md, „Fake”).
//!
//! [`FakeTransfer`] używa **tego samego silnika** co `transfer-impl`
//! (`transfer_contract::engine`), ale paczki trzyma w pamięci (klucz: ścieżka), „szyfrowanie”
//! to skrót hasła, a zegar jest wirtualny. Dodatkowo: skryptowane błędy ([`FakeTransfer::fail_next`]),
//! rejestr zdarzeń, porty w pamięci ([`MemoryDocumentStore`]) i porty zawodzące po N zapisach
//! ([`FailureBudget`], [`FlakySessions`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod flaky;
mod ports;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use accounts_hub_contract::SecretString;
use transfer_contract::backup::{
    BACKUP_PREFIX, DEFAULT_SNAPSHOTS_KEEP, EXTENSION, SNAPSHOT_PREFIX, rotation_victims,
    stamped_name,
};
use transfer_contract::engine::{Engine, ExportSpec};
use transfer_contract::{
    BackupReport, BackupRequest, EncryptionInfo, ExportReport, ExportRequest, ImportOptions,
    ImportReport, Inspection, MANIFEST_PATH, Manifest, MemoryPackage, MemorySource, PackageKind,
    RollbackReport, SnapshotId, SnapshotInfo, Transfer, TransferError, TransferPorts, events,
    sha256_hex, validate_password,
};

pub use flaky::FlakySessions;
pub use ports::{FailureBudget, MemoryDocumentStore, SeqIds, VirtualClock};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Paczka w pamięci atrapy.
#[derive(Debug, Clone)]
struct Stored {
    manifest: Manifest,
    package: MemoryPackage,
    /// SHA-256 hasła (atrapa nie szyfruje naprawdę).
    password: Option<String>,
}

#[derive(Debug, Default)]
struct State {
    packages: BTreeMap<PathBuf, Stored>,
    snapshots: BTreeMap<SnapshotId, Stored>,
    fail_next: Option<TransferError>,
    events: Vec<(String, serde_json::Value)>,
}

/// Atrapa modułu `transfer` (paczki w pamięci).
pub struct FakeTransfer {
    ports: TransferPorts,
    state: Mutex<State>,
}

fn fake_encryption() -> EncryptionInfo {
    EncryptionInfo {
        scheme: "fake".to_owned(),
        kdf: "sha256".to_owned(),
        salt: None,
        nonce: String::new(),
    }
}

fn password_hash(p: &SecretString) -> String {
    sha256_hex(p.expose_secret().as_bytes())
}

impl FakeTransfer {
    /// Atrapa na podanych portach.
    pub fn new(ports: TransferPorts) -> Self {
        Self {
            ports,
            state: Mutex::new(State::default()),
        }
    }

    /// Porty.
    pub fn ports(&self) -> &TransferPorts {
        &self.ports
    }

    /// Następna operacja zwróci ten błąd (jednorazowo).
    pub fn fail_next(&self, error: TransferError) {
        lock(&self.state).fail_next = Some(error);
    }

    /// Zarejestrowane zdarzenia `(rodzaj, ładunek)`.
    pub fn events(&self) -> Vec<(String, serde_json::Value)> {
        lock(&self.state).events.clone()
    }

    /// Wszystkie wpisy paczki jawnej (z manifestem) — do testów szpiegowskich.
    pub fn entries(&self, path: &Path) -> Vec<(String, Vec<u8>)> {
        let st = lock(&self.state);
        let Some(stored) = st.packages.get(path) else {
            return Vec::new();
        };
        let mut out = vec![(
            MANIFEST_PATH.to_owned(),
            serde_json::to_vec(&stored.manifest).unwrap_or_default(),
        )];
        out.extend(stored.package.entries.clone());
        out
    }

    /// Paczki w katalogu.
    pub fn packages_in(&self, dir: &Path) -> Vec<PathBuf> {
        lock(&self.state)
            .packages
            .keys()
            .filter(|p| p.parent() == Some(dir))
            .cloned()
            .collect()
    }

    fn take_failure(&self) -> Result<(), TransferError> {
        match lock(&self.state).fail_next.take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn emit(&self, kind: &str, payload: serde_json::Value) {
        lock(&self.state).events.push((kind.to_owned(), payload));
    }

    fn open(
        &self,
        path: &Path,
        password: Option<&SecretString>,
    ) -> Result<MemorySource, TransferError> {
        let stored = lock(&self.state)
            .packages
            .get(path)
            .cloned()
            .ok_or_else(|| TransferError::NotFound {
                what: path.display().to_string(),
            })?;
        Self::unlock(stored, password, &self.ports)
    }

    fn unlock(
        stored: Stored,
        password: Option<&SecretString>,
        ports: &TransferPorts,
    ) -> Result<MemorySource, TransferError> {
        match (&stored.password, password) {
            (Some(_), None) => return Err(TransferError::PasswordRequired),
            (Some(hash), Some(p)) if *hash != password_hash(p) => {
                return Err(TransferError::WrongPassword);
            }
            _ => {}
        }
        stored.manifest.validate(&ports.limits)?;
        Ok(MemorySource {
            manifest: stored.manifest,
            package: stored.package,
            migrations: Vec::new(),
        })
    }

    fn write(
        &self,
        dest: &Path,
        kind: PackageKind,
        request_scope: &ExportRequest,
    ) -> Result<ExportReport, TransferError> {
        if let Some(p) = &request_scope.password {
            validate_password(p)?;
        }
        let encryption = request_scope.password.as_ref().map(|_| fake_encryption());
        let spec = ExportSpec {
            kind,
            scope: &request_scope.scope,
            encryption: encryption.as_ref(),
            notes: request_scope.notes.as_deref(),
            cancel: request_scope.cancel.as_ref(),
        };
        let mut package = MemoryPackage::default();
        let outcome = Engine::new(&self.ports).export(&spec, &mut package)?;
        let bytes = outcome.manifest.total_bytes();
        let stored = Stored {
            manifest: outcome.manifest.clone(),
            package,
            password: request_scope.password.as_ref().map(password_hash),
        };
        lock(&self.state)
            .packages
            .insert(dest.to_path_buf(), stored);
        self.emit(
            events::EXPORT_COMPLETED,
            serde_json::json!({ "kind": kind.as_str(), "entries": outcome.manifest.content.len() }),
        );
        Ok(ExportReport {
            path: dest.to_path_buf(),
            file_bytes: bytes,
            manifest: outcome.manifest,
            warnings: outcome.warnings,
        })
    }
}

impl Transfer for FakeTransfer {
    fn export(&self, request: &ExportRequest) -> Result<ExportReport, TransferError> {
        self.take_failure()?;
        if !matches!(request.kind, PackageKind::Export | PackageKind::Backup) {
            return Err(TransferError::invalid(
                "kind",
                "eksport obsługuje tylko export/backup",
            ));
        }
        self.write(&request.dest, request.kind, request)
    }

    fn export_secrets(
        &self,
        dest: &Path,
        password: &SecretString,
    ) -> Result<ExportReport, TransferError> {
        self.take_failure()?;
        validate_password(password)?;
        let mut package = MemoryPackage::default();
        let outcome = Engine::new(&self.ports).export_secrets(&mut package, &fake_encryption())?;
        let stored = Stored {
            manifest: outcome.manifest.clone(),
            package,
            password: Some(password_hash(password)),
        };
        lock(&self.state)
            .packages
            .insert(dest.to_path_buf(), stored);
        Ok(ExportReport {
            path: dest.to_path_buf(),
            file_bytes: outcome.manifest.total_bytes(),
            manifest: outcome.manifest,
            warnings: outcome.warnings,
        })
    }

    fn inspect(
        &self,
        package: &Path,
        options: &ImportOptions,
    ) -> Result<Inspection, TransferError> {
        self.take_failure()?;
        let mut source = self.open(package, options.password.as_ref())?;
        let plan = Engine::new(&self.ports).plan(&mut source, options)?;
        self.emit(
            events::IMPORT_DRY_RUN,
            serde_json::json!({ "items": plan.report.items.len(), "writes": plan.report.writes() }),
        );
        Ok(Inspection {
            manifest: source.manifest,
            report: plan.report,
        })
    }

    fn import(
        &self,
        package: &Path,
        options: &ImportOptions,
    ) -> Result<ImportReport, TransferError> {
        self.take_failure()?;
        let engine = Engine::new(&self.ports);
        let mut source = self.open(package, options.password.as_ref())?;
        let plan = engine.plan(&mut source, options)?;
        let mut snapshot = None;
        if plan.report.writes() > 0 {
            let mut snap = MemoryPackage::default();
            let outcome = engine.snapshot(&plan, &mut snap, Some(&fake_encryption()))?;
            let id = stamped_name(SNAPSHOT_PREFIX, self.ports.clock.now());
            let mut st = lock(&self.state);
            st.snapshots.insert(
                id.clone(),
                Stored {
                    manifest: outcome.manifest,
                    package: snap,
                    password: None,
                },
            );
            let ids: Vec<String> = st.snapshots.keys().cloned().collect();
            for victim in rotation_victims(SNAPSHOT_PREFIX, &ids, DEFAULT_SNAPSHOTS_KEEP) {
                st.snapshots.remove(&victim);
            }
            drop(st);
            self.emit(
                events::IMPORT_SNAPSHOT_CREATED,
                serde_json::json!({ "snapshot": id }),
            );
            snapshot = Some(id);
        }
        let mut report = engine.apply(&mut source, &plan, options.cancel.as_ref())?;
        report.snapshot = snapshot;
        self.emit(
            events::IMPORT_COMPLETED,
            serde_json::json!({ "added": report.added, "failed": report.failed }),
        );
        Ok(report)
    }

    fn snapshots(&self) -> Result<Vec<SnapshotInfo>, TransferError> {
        self.take_failure()?;
        Ok(lock(&self.state)
            .snapshots
            .iter()
            .map(|(id, s)| SnapshotInfo {
                id: id.clone(),
                created_at: s.manifest.created_at,
                items: s.manifest.content.len() as u64,
            })
            .collect())
    }

    fn rollback(&self, snapshot: &SnapshotId) -> Result<RollbackReport, TransferError> {
        self.take_failure()?;
        let stored = lock(&self.state)
            .snapshots
            .get(snapshot)
            .cloned()
            .ok_or_else(|| TransferError::NotFound {
                what: format!("snapshot {snapshot}"),
            })?;
        let mut source = Self::unlock(stored, None, &self.ports)?;
        let report = Engine::new(&self.ports).rollback(&mut source)?;
        self.emit(
            events::ROLLED_BACK,
            serde_json::json!({ "snapshot": snapshot, "restored": report.restored }),
        );
        Ok(report)
    }

    fn backup(&self, request: &BackupRequest) -> Result<BackupReport, TransferError> {
        self.take_failure()?;
        let name = stamped_name(BACKUP_PREFIX, self.ports.clock.now());
        let dest = request.dir.join(format!("{name}.{EXTENSION}"));
        let export = ExportRequest {
            scope: request.scope.clone(),
            dest: dest.clone(),
            kind: PackageKind::Backup,
            password: request.password.clone(),
            notes: None,
            cancel: request.cancel.clone(),
        };
        let export = self.write(&dest, PackageKind::Backup, &export)?;
        let stems: Vec<String> = self
            .packages_in(&request.dir)
            .iter()
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        let mut rotated_out = Vec::new();
        for victim in rotation_victims(BACKUP_PREFIX, &stems, request.keep.max(1)) {
            let path = request.dir.join(format!("{victim}.{EXTENSION}"));
            lock(&self.state).packages.remove(&path);
            rotated_out.push(path);
        }
        self.emit(
            events::BACKUP_COMPLETED,
            serde_json::json!({ "rotated_out": rotated_out.len() }),
        );
        Ok(BackupReport {
            export,
            rotated_out,
        })
    }
}
