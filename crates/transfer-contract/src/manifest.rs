//! `manifest.json` paczki `.alfa` (docs/formats/alfa-package.md §3) i sumy kontrolne.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sessions_contract::SessionId;
use sha2::{Digest, Sha256};

use crate::error::TransferError;
use crate::paths::{MANIFEST_PATH, validate_entry_path};

/// Wersja schematu paczki zapisywana przez tę wersję Alfy.
pub const SCHEMA_VERSION: &str = "1.0.0";

/// Wersja schematu jako `semver::Version`.
pub fn schema_version() -> semver::Version {
    semver::Version::new(1, 0, 0)
}

/// Rodzaj paczki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    /// Eksport ręczny.
    Export,
    /// Kopia zapasowa (zaplanowany eksport, ten sam kod).
    Backup,
    /// Paczka sekretów ze starszej wersji (tylko odczyt manifestu — import odmawia; nowych nie
    /// ma, CX-a).
    Secrets,
    /// Snapshot przed importem (lokalny, do rollbacku; szyfrowany kluczem maszyny).
    Snapshot,
}

impl PackageKind {
    /// Nazwa w nagłówku i manifeście.
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Export => "export",
            PackageKind::Backup => "backup",
            PackageKind::Secrets => "secrets",
            PackageKind::Snapshot => "snapshot",
        }
    }
}

/// Maszyna źródłowa — **bez danych osobowych**: `id` to skrót z `device-profile` (nie
/// `MachineGuid`), `name` to etykieta nadana przez użytkownika (np. `desktop`), nie nazwa hosta
/// ani konta.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MachineInfo {
    /// Identyfikator maszyny (32 znaki hex z `device-profile`).
    pub id: String,
    /// Etykieta maszyny nadana przez użytkownika.
    pub name: String,
    /// System (np. `Windows 11 Pro 24H2`).
    pub os: String,
    /// Klasa sprzętu (np. `standard-amd`, `laptop-cuda`).
    pub hw_class: String,
}

/// Liczniki zawartości.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Counts {
    /// Sesje.
    pub sessions: u64,
    /// Tury (wszystkie gałęzie).
    pub turns: u64,
    /// Dokumenty (konfiguracja, agentki, obsady…).
    pub documents: u64,
    /// Wpisy pamięci (linie NDJSON).
    pub memory_entries: u64,
    /// Artefakty.
    pub artifacts: u64,
    /// Sekrety — zawsze 0 w nowych paczkach (pole zgodności ze starszymi; CX-a).
    pub secrets: u64,
}

/// Co weszło do paczki.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScopeSummary {
    /// Klucze zakresu (`config.common`, `sessions`…), posortowane.
    pub keys: Vec<String>,
    /// Identyfikatory sesji w paczce.
    pub sessions: Vec<SessionId>,
    /// Liczniki.
    pub counts: Counts,
}

/// Wpis listy zawartości.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContentEntry {
    /// Ścieżka w archiwum.
    pub path: String,
    /// SHA-256 (hex, małe litery).
    pub sha256: String,
    /// Rozmiar w bajtach.
    pub bytes: u64,
}

impl ContentEntry {
    /// Wpis dla treści `data`.
    pub fn of(path: &str, data: &[u8]) -> Self {
        Self {
            path: path.to_owned(),
            sha256: sha256_hex(data),
            bytes: data.len() as u64,
        }
    }
}

/// Parametry szyfrowania (informacyjnie w manifeście; obowiązujące są w nagłówku `ALFAENC1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EncryptionInfo {
    /// Schemat (np. `xchacha20poly1305-stream`).
    pub scheme: String,
    /// Wyprowadzenie klucza (`argon2id` z hasła albo `machine-key`).
    pub kdf: String,
    /// Sól (hex), jeśli jest.
    pub salt: Option<String>,
    /// Prefiks nonce (hex).
    pub nonce: String,
}

/// Manifest paczki (schemat v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    /// Wersja schematu paczki (semver).
    #[schemars(with = "String")]
    pub schema_version: semver::Version,
    /// Wersja Alfy, która wykonała eksport.
    #[schemars(with = "String")]
    pub app_version: semver::Version,
    /// Rodzaj.
    pub kind: PackageKind,
    /// Data eksportu (UTC).
    pub created_at: DateTime<Utc>,
    /// Maszyna źródłowa.
    pub source_machine: MachineInfo,
    /// Zakres.
    pub scope: ScopeSummary,
    /// Lista **wszystkich** plików poza manifestem.
    pub content: Vec<ContentEntry>,
    /// Skrót nad posortowaną listą `content` ([`content_sha256`]).
    pub content_sha256: String,
    /// Szyfrowanie (`None` = paczka jawna).
    pub encryption: Option<EncryptionInfo>,
    /// Opis od użytkownika.
    pub notes: Option<String>,
    /// Liczba zredagowanych ciągów wyglądających na sekrety (strażnik eksportu).
    #[serde(default)]
    pub redactions: u64,
}

/// Limity odczytu paczki (ochrona przed zip-bomb i obciętymi/złośliwymi archiwami).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Limits {
    /// Maksymalna liczba wpisów archiwum.
    pub max_entries: u64,
    /// Maksymalny rozmiar jednego wpisu po rozpakowaniu.
    pub max_entry_bytes: u64,
    /// Maksymalny łączny rozmiar po rozpakowaniu.
    pub max_total_bytes: u64,
    /// Maksymalny rozmiar manifestu.
    pub max_manifest_bytes: u64,
    /// Maksymalny stopień kompresji wpisu (rozpakowany / spakowany) dla wpisów > 1 MiB.
    pub max_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 200_000,
            max_entry_bytes: 2 << 30,
            max_total_bytes: 16 << 30,
            max_manifest_bytes: 64 << 20,
            max_ratio: 1_000,
        }
    }
}

/// SHA-256 jako hex (małe litery).
pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

/// Bajty jako hex (małe litery).
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// Skrót listy zawartości: SHA-256 nad liniami `ścieżka\tsha256\tbajty\n` posortowanymi po ścieżce.
pub fn content_sha256(content: &[ContentEntry]) -> String {
    let mut sorted: Vec<&ContentEntry> = content.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut hasher = Sha256::new();
    for e in sorted {
        hasher.update(format!("{}\t{}\t{}\n", e.path, e.sha256, e.bytes).as_bytes());
    }
    hex(&hasher.finalize())
}

impl Manifest {
    /// Wpis listy zawartości o danej ścieżce.
    pub fn entry(&self, path: &str) -> Option<&ContentEntry> {
        self.content.iter().find(|e| e.path == path)
    }

    /// Łączny rozmiar zawartości (bajty po rozpakowaniu).
    pub fn total_bytes(&self) -> u64 {
        self.content.iter().map(|e| e.bytes).sum()
    }

    /// Walidacja: obsługiwana wersja, bezpieczne i unikalne ścieżki, zgodny skrót listy, limity.
    pub fn validate(&self, limits: &Limits) -> Result<(), TransferError> {
        let supported = schema_version();
        if self.schema_version.major != supported.major || self.schema_version > supported {
            return Err(TransferError::NewerSchema {
                found: self.schema_version.to_string(),
                supported: supported.to_string(),
            });
        }
        let count = self.content.len() as u64 + 1;
        if count > limits.max_entries {
            return Err(limit("liczba wpisów", count, limits.max_entries));
        }
        let mut seen = BTreeSet::new();
        for e in &self.content {
            validate_entry_path(&e.path).map_err(|reason| TransferError::UnsafePath {
                path: e.path.clone(),
                reason,
            })?;
            if e.path == MANIFEST_PATH || !seen.insert(e.path.as_str()) {
                return Err(TransferError::corrupt(format!(
                    "powtórzony wpis `{}`",
                    e.path
                )));
            }
            if e.bytes > limits.max_entry_bytes {
                return Err(limit(&e.path, e.bytes, limits.max_entry_bytes));
            }
            if e.sha256.len() != 64 || !e.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(TransferError::invalid(MANIFEST_PATH, "zła suma SHA-256"));
            }
        }
        let total = self.total_bytes();
        if total > limits.max_total_bytes {
            return Err(limit("łączny rozmiar", total, limits.max_total_bytes));
        }
        if content_sha256(&self.content) != self.content_sha256 {
            return Err(TransferError::Checksum {
                path: MANIFEST_PATH.to_owned(),
            });
        }
        Ok(())
    }
}

/// Błąd przekroczenia limitu.
pub fn limit(what: &str, actual: u64, max: u64) -> TransferError {
    TransferError::LimitExceeded {
        what: what.to_owned(),
        actual,
        max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Manifest {
        let content = vec![
            ContentEntry::of("config/common/shared.toml", b"a = 1\n"),
            ContentEntry::of("sessions/s1/session.json", b"{}"),
        ];
        Manifest {
            schema_version: schema_version(),
            app_version: semver::Version::new(0, 0, 1),
            kind: PackageKind::Export,
            created_at: DateTime::UNIX_EPOCH,
            source_machine: MachineInfo::default(),
            scope: ScopeSummary::default(),
            content_sha256: content_sha256(&content),
            content,
            encryption: None,
            notes: None,
            redactions: 0,
        }
    }

    #[test]
    fn sha_and_content_hash_are_order_independent() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let m = sample();
        let mut reversed = m.content.clone();
        reversed.reverse();
        assert_eq!(content_sha256(&reversed), m.content_sha256);
        assert_eq!(m.validate(&Limits::default()), Ok(()));
    }

    #[test]
    fn validation_rejects_bad_manifests() {
        let limits = Limits::default();
        let mut newer = sample();
        newer.schema_version = semver::Version::new(1, 1, 0);
        assert!(matches!(
            newer.validate(&limits),
            Err(TransferError::NewerSchema { .. })
        ));
        let mut tampered = sample();
        tampered.content[0].bytes += 1;
        assert!(matches!(
            tampered.validate(&limits),
            Err(TransferError::Checksum { .. })
        ));
        let mut evil = sample();
        evil.content[0].path = "../evil".into();
        evil.content_sha256 = content_sha256(&evil.content);
        assert!(matches!(
            evil.validate(&limits),
            Err(TransferError::UnsafePath { .. })
        ));
        let mut dup = sample();
        dup.content[1].path = dup.content[0].path.clone();
        dup.content_sha256 = content_sha256(&dup.content);
        assert!(matches!(
            dup.validate(&limits),
            Err(TransferError::Corrupt { .. })
        ));
        let tight = Limits {
            max_total_bytes: 3,
            ..Limits::default()
        };
        assert!(matches!(
            sample().validate(&tight),
            Err(TransferError::LimitExceeded { .. })
        ));
    }
}
