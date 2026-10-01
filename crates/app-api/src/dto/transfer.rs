//! DTO importu/eksportu `.alfa` (odpowiedniki `types-hub.ts`; realizuje moduł `transfer`).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::common::Iso8601;
use super::hub::SecretInput;

/// Zakres eksportu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportScope {
    pub config_common: bool,
    pub personas: bool,
    pub casts: bool,
    pub sessions: Vec<String>,
    pub artifacts: bool,
    pub logs: bool,
    pub config_machine: bool,
}

/// Żądanie eksportu. Hasło nigdy nie trafia do logów (`Debug` redaguje).
#[derive(Clone, Deserialize)]
pub struct ExportRequest {
    pub scope: ExportScope,
    pub password: Option<SecretInput>,
}

impl fmt::Debug for ExportRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExportRequest")
            .field("scope", &self.scope)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .finish()
    }
}

/// Wynik eksportu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ExportResult {
    Cancelled,
    Saved {
        path: String,
        files: u64,
        bytes: u64,
    },
}

/// Różnica elementu paczki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemDiff {
    New,
    Same,
    Changed,
    Collision,
}

/// Rodzaj elementu paczki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DryRunKind {
    Session,
    Config,
    Persona,
    Cast,
}

/// Element podglądu importu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DryRunItem {
    pub key: String,
    pub kind: DryRunKind,
    pub label: String,
    pub diff: ItemDiff,
}

/// Manifest paczki.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifestSummary {
    pub schema_version: String,
    pub app_version: String,
    pub created_at: Iso8601,
    pub source_machine: String,
    pub encrypted: bool,
}

/// Wynik podglądu paczki.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InspectResult {
    Cancelled,
    NeedsPassword {
        path: String,
    },
    Inspected {
        path: String,
        manifest: PackageManifestSummary,
        items: Vec<DryRunItem>,
        warnings: Vec<String>,
        migrations: Vec<String>,
    },
}

/// Tryb importu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    Add,
    Merge,
    Replace,
}

/// Rozwiązanie kolizji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollisionResolution {
    KeepLocal,
    TakeImported,
    KeepBoth,
}

/// Żądanie importu.
#[derive(Clone, Deserialize)]
pub struct ImportRequest {
    pub path: String,
    pub mode: ImportMode,
    pub resolutions: BTreeMap<String, CollisionResolution>,
    pub password: Option<SecretInput>,
}

impl fmt::Debug for ImportRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImportRequest")
            .field("path", &self.path)
            .field("mode", &self.mode)
            .field("resolutions", &self.resolutions)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .finish()
    }
}

/// Wynik importu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportResult {
    pub snapshot_id: String,
    pub imported: u64,
    pub skipped: u64,
}
