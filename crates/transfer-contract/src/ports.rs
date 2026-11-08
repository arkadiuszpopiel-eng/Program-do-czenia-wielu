//! Porty: skąd eksport czyta i dokąd import zapisuje. Moduł `transfer` nie zna plików innych
//! modułów — kompozycja buildu podpina adaptery (konfiguracja, agentki, pamięć…) jako
//! [`DocumentStore`], sesje przez `sessions-contract`, sekrety przez `accounts-hub-contract`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use accounts_hub_contract::SecretStore;
use chrono::{DateTime, Utc};
use sessions_contract::{SessionId, Sessions};

use crate::error::TransferError;
use crate::manifest::{Limits, MachineInfo, Manifest};
use crate::report::UpcastStep;
use crate::scope::Category;

/// Magazyn dokumentów jednej kategorii (pliki konfiguracji, agentki, obsady, pamięć…).
/// Nazwy są względne, z `/` jako separatorem i spełniają [`crate::validate_entry_path`].
pub trait DocumentStore: Send + Sync {
    /// Nazwy wszystkich dokumentów (posortowane).
    fn list(&self) -> Result<Vec<String>, TransferError>;
    /// Treść dokumentu; `None`, gdy nie istnieje.
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, TransferError>;
    /// Zapis **atomowy** (całość albo nic: plik tymczasowy + zamiana).
    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), TransferError>;
    /// Usuwa dokument; `true`, gdy istniał.
    fn remove(&self, name: &str) -> Result<bool, TransferError>;
}

/// Zegar (wirtualny w atrapie).
pub trait Clock: Send + Sync {
    /// Teraz (UTC).
    fn now(&self) -> DateTime<Utc>;
}

/// Zegar systemowy.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Źródło nowych identyfikatorów sesji (kopie przy kolizji).
pub trait IdSource: Send + Sync {
    /// Nowy, unikalny identyfikator sesji (spełniający `is_portable_session_id`).
    fn new_session_id(&self) -> SessionId;
}

/// Porty modułu (wspólne dla `-impl` i `-fake`).
#[derive(Clone)]
pub struct TransferPorts {
    /// Sesje (`None` = instalacja bez modułu sesji; elementy sesji są pomijane z ostrzeżeniem).
    pub sessions: Option<Arc<dyn Sessions>>,
    /// Magazyny dokumentów per kategoria.
    pub documents: BTreeMap<Category, Arc<dyn DocumentStore>>,
    /// Magazyn sekretów (strażnik eksportu, eksport/import sekretów, klucz snapshotów).
    pub secrets: Option<Arc<dyn SecretStore>>,
    /// Ta maszyna.
    pub machine: MachineInfo,
    /// Wersja Alfy.
    pub app_version: semver::Version,
    /// Korzeń katalogów roboczych sesji (`%USERPROFILE%\Alfa\Sesje`) — ścieżki w paczce są
    /// względne (bez nazwy konta Windows).
    pub workdir_root: Option<PathBuf>,
    /// Zegar.
    pub clock: Arc<dyn Clock>,
    /// Identyfikatory kopii.
    pub ids: Arc<dyn IdSource>,
    /// Limity odczytu paczek.
    pub limits: Limits,
}

impl TransferPorts {
    /// Magazyn kategorii.
    pub fn store(&self, category: Category) -> Option<&Arc<dyn DocumentStore>> {
        self.documents.get(&category)
    }

    /// Sesje albo błąd „nieobsługiwane”.
    pub fn sessions(&self) -> Result<&Arc<dyn Sessions>, TransferError> {
        self.sessions
            .as_ref()
            .ok_or_else(|| TransferError::Unsupported {
                category: Category::Sessions.key().to_owned(),
            })
    }
}

/// Zapis zawartości paczki (kontener dopisuje manifest jako pierwszy wpis na końcu zapisu).
pub trait PackageSink {
    /// Dodaje wpis (ścieżka już zwalidowana przez silnik).
    fn add(&mut self, path: &str, bytes: &[u8]) -> Result<(), TransferError>;
}

/// Odczyt otwartej i zweryfikowanej paczki (manifest po migracji, limity, ścieżki).
pub trait PackageSource {
    /// Manifest (bieżąca wersja schematu).
    fn manifest(&self) -> &Manifest;
    /// Migracje zastosowane do manifestu.
    fn migrations(&self) -> Vec<UpcastStep> {
        Vec::new()
    }
    /// Treść wpisu z weryfikacją sumy i rozmiaru względem manifestu; `None`, gdy manifest nie
    /// zawiera ścieżki.
    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, TransferError>;
}

/// Paczka w pamięci (atrapa, testy, snapshot w trakcie budowy).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryPackage {
    /// Wpisy (ścieżka → treść).
    pub entries: BTreeMap<String, Vec<u8>>,
}

impl PackageSink for MemoryPackage {
    fn add(&mut self, path: &str, bytes: &[u8]) -> Result<(), TransferError> {
        if self
            .entries
            .insert(path.to_owned(), bytes.to_vec())
            .is_some()
        {
            return Err(TransferError::corrupt(format!("powtórzony wpis `{path}`")));
        }
        Ok(())
    }
}

/// Odczyt [`MemoryPackage`] z manifestem (weryfikuje sumy jak kontener ZIP).
#[derive(Debug, Clone)]
pub struct MemorySource {
    /// Manifest.
    pub manifest: Manifest,
    /// Wpisy.
    pub package: MemoryPackage,
    /// Migracje.
    pub migrations: Vec<UpcastStep>,
}

impl PackageSource for MemorySource {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn migrations(&self) -> Vec<UpcastStep> {
        self.migrations.clone()
    }

    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, TransferError> {
        let Some(expected) = self.manifest.entry(path) else {
            return Ok(None);
        };
        let bytes = self
            .package
            .entries
            .get(path)
            .ok_or_else(|| TransferError::corrupt(format!("brak wpisu `{path}`")))?;
        if crate::manifest::ContentEntry::of(path, bytes) != *expected {
            return Err(TransferError::Checksum {
                path: path.to_owned(),
            });
        }
        Ok(Some(bytes.clone()))
    }
}
