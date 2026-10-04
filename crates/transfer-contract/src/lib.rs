//! Kontrakt modułu `transfer` (docs/modules/transfer/SPEC.md, docs/formats/alfa-package.md,
//! PLAN §15.1): import/eksport paczek `.alfa` między maszynami właściciela i kopie zapasowe
//! jako zaplanowany eksport.
//!
//! Zawiera: trait [`Transfer`], manifest i limity ([`Manifest`], [`Limits`]), walidację ścieżek
//! wpisów ([`validate_entry_path`] — ochrona przed zip-slip), format przenośny sesji
//! ([`portable`]), upcastery ([`migrate`]), strażnika sekretów ([`SecretGuard`]), scalanie
//! dokumentów ([`docmerge`]) i **silnik** ([`engine::Engine`]) wspólny dla `-impl` i `-fake`:
//! eksport z portów, plan (dry-run), snapshot, zapis per element, rollback.
//!
//! Operacje są synchroniczne (pliki, SQLite); wywołujący z kodu async używa `spawn_blocking`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod backup;
pub mod docmerge;
pub mod engine;
mod error;
pub mod guard;
mod manifest;
pub mod migrate;
mod paths;
pub mod portable;
mod ports;
mod report;
mod scope;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use std::path::Path;

pub use backup::{BackgroundState, BackupRequest, BackupSchedule};
pub use error::TransferError;
pub use guard::SecretGuard;
pub use manifest::{
    ContentEntry, Counts, EncryptionInfo, Limits, MachineInfo, Manifest, PackageKind,
    SCHEMA_VERSION, ScopeSummary, content_sha256, hex, limit, schema_version, sha256_hex,
};
pub use paths::{
    EntryKind, MANIFEST_PATH, MAX_DEPTH, MAX_PATH_BYTES, PathError, ROLLBACK_PATH, SECRETS_PATH,
    SESSION_FILE, TURNS_FILE, classify, document_path, session_path, validate_entry_path,
};
pub use ports::{
    Clock, DocumentStore, IdSource, MemoryPackage, MemorySource, PackageSink, PackageSource,
    SystemClock, TransferPorts,
};
pub use report::{
    BackupReport, DryRunReport, ExportReport, ImportReport, Inspection, ItemDiff, ItemOutcome,
    ItemRef, ItemState, Outcome, PlannedAction, RollbackReport, SnapshotId, SnapshotInfo,
    UpcastStep, Warning,
};
pub use scope::{
    CancelToken, Category, CollisionResolution, ExportRequest, ExportScope, ImportMode,
    ImportOptions, MIN_PASSWORD_CHARS, ModeMap, Selection, validate_password,
};

/// Nazwy zdarzeń modułu (Audyt). Ładunki bez treści elementów i bez sekretów.
pub mod events {
    /// Rozpoczęto eksport (`{ "kind", "path" }`).
    pub const EXPORT_STARTED: &str = "transfer.export.started";
    /// Zakończono eksport (`{ "kind", "path", "entries", "bytes", "redactions" }`).
    pub const EXPORT_COMPLETED: &str = "transfer.export.completed";
    /// Wykonano dry-run (`{ "items", "writes", "collisions" }`).
    pub const IMPORT_DRY_RUN: &str = "transfer.import.dry_run";
    /// Utworzono snapshot przed importem (`{ "snapshot", "items" }`).
    pub const IMPORT_SNAPSHOT_CREATED: &str = "transfer.import.snapshot_created";
    /// Zakończono import (`{ "snapshot", "added", "merged", "replaced", "copied", "failed" }`).
    pub const IMPORT_COMPLETED: &str = "transfer.import.completed";
    /// Wykonano rollback (`{ "snapshot", "restored", "removed" }`).
    pub const ROLLED_BACK: &str = "transfer.rolled_back";
    /// Zakończono kopię zapasową (`{ "path", "rotated_out" }`).
    pub const BACKUP_COMPLETED: &str = "transfer.backup.completed";
}

/// Kontrakt modułu `transfer`.
pub trait Transfer: Send + Sync {
    /// Eksport zakresu do pliku `.alfa` (`kind` = `Export` albo `Backup`). **Sekrety nigdy** nie
    /// trafiają do paczki (strażnik + skan końcowy). Hasło → szyfrowanie całej paczki. Eksportu
    /// sekretów nie ma (AGENTS.md: sekrety tylko w Credential Manager — CX-a); paczka sekretów
    /// ze starszej wersji jest przy imporcie odrzucana, sekcja `secrets.json` pomijana.
    fn export(&self, request: &ExportRequest) -> Result<ExportReport, TransferError>;

    /// Podgląd: manifest + dry-run (nic nie zapisuje).
    fn inspect(&self, package: &Path, options: &ImportOptions)
    -> Result<Inspection, TransferError>;

    /// Import: dry-run → automatyczny snapshot dotkniętych elementów → zapis per element.
    fn import(
        &self,
        package: &Path,
        options: &ImportOptions,
    ) -> Result<ImportReport, TransferError>;

    /// Snapshoty (najnowsze na końcu).
    fn snapshots(&self) -> Result<Vec<SnapshotInfo>, TransferError>;

    /// Rollback jednym kliknięciem: stan elementów sprzed importu.
    fn rollback(&self, snapshot: &SnapshotId) -> Result<RollbackReport, TransferError>;

    /// Kopia zapasowa = eksport `kind = backup` do katalogu + rotacja N ostatnich.
    fn backup(&self, request: &BackupRequest) -> Result<BackupReport, TransferError>;
}
