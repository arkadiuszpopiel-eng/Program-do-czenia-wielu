//! Raporty: dry-run (podgląd różnic), wynik importu, eksportu, snapshotu, rollbacku i kopii.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sessions_contract::SessionId;

use crate::manifest::Manifest;
use crate::scope::Category;

/// Element paczki (bez treści).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "item", rename_all = "snake_case")]
pub enum ItemRef {
    /// Dokument kategorii.
    Document {
        /// Kategoria.
        category: Category,
        /// Nazwa względem katalogu kategorii.
        name: String,
    },
    /// Sesja.
    Session {
        /// Identyfikator (z paczki; po kopii — patrz `id_map`).
        id: SessionId,
    },
    /// Sekret (tylko nazwa — nigdy wartość).
    Secret {
        /// Nazwa w magazynie.
        name: String,
    },
}

impl ItemRef {
    /// Kategoria elementu.
    pub fn category(&self) -> Category {
        match self {
            ItemRef::Document { category, .. } => *category,
            ItemRef::Session { .. } => Category::Sessions,
            ItemRef::Secret { .. } => Category::Secrets,
        }
    }
}

impl std::fmt::Display for ItemRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemRef::Document { category, name } => write!(f, "{}/{name}", category.dir()),
            ItemRef::Session { id } => write!(f, "sesja {id}"),
            ItemRef::Secret { name } => write!(f, "sekret {name}"),
        }
    }
}

/// Stan elementu paczki względem stanu lokalnego.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// Nie ma lokalnie.
    New,
    /// Identyczny.
    Same,
    /// Różny (dla sesji: jedna wersja jest kontynuacją drugiej).
    Changed,
    /// Ta sama sesja (`id`) z rozbieżną historią z dwóch maszyn.
    Collision,
}

/// Zaplanowana czynność.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PlannedAction {
    /// Nic do zrobienia (identyczne).
    Keep,
    /// Dodanie nowego elementu.
    Add,
    /// Scalenie.
    Merge,
    /// Zastąpienie w całości.
    Replace,
    /// Import jako kopia z nowym identyfikatorem.
    Copy {
        /// Nowy identyfikator.
        new_id: SessionId,
    },
    /// Pominięcie.
    Skip {
        /// Powód.
        reason: String,
    },
}

impl PlannedAction {
    /// Czy czynność zmienia stan lokalny.
    pub fn writes(&self) -> bool {
        !matches!(self, PlannedAction::Keep | PlannedAction::Skip { .. })
    }
}

/// Pozycja podglądu różnic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ItemDiff {
    /// Element.
    pub item: ItemRef,
    /// Stan względem lokalnego.
    pub state: ItemState,
    /// Czynność.
    pub action: PlannedAction,
    /// Rozmiar w paczce (bajty).
    pub bytes: u64,
    /// Szczegóły (np. „+12 tur”, tytuł sesji).
    pub detail: Option<String>,
}

/// Ostrzeżenie dry-run/importu/eksportu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "warning", rename_all = "snake_case")]
pub enum Warning {
    /// Nieznany wpis paczki — pominięty.
    UnknownEntry {
        /// Ścieżka.
        path: String,
    },
    /// Klucze polityk Jądra (`kernel.*`) pominięte — zmienia je tylko Broker.
    KernelKeysSkipped {
        /// Dokument.
        name: String,
        /// Klucze.
        keys: Vec<String>,
    },
    /// Nakładka maszyny pominięta (brak jawnej zgody).
    MachineOverlaySkipped {
        /// Dokument.
        name: String,
    },
    /// Nakładka z maszyny innej klasy sprzętu.
    HardwareClassDiffers {
        /// Klasa w paczce.
        package: String,
        /// Klasa tej maszyny.
        local: String,
    },
    /// Sesja prywatna — pominięta (brak jawnego wyboru) albo zaimportowana.
    PrivateSession {
        /// Sesja.
        id: SessionId,
        /// Czy pominięta.
        skipped: bool,
    },
    /// Brak magazynu dla kategorii w tej instalacji — elementy pominięte.
    CategoryUnavailable {
        /// Kategoria.
        category: Category,
    },
    /// Dokumentu nie da się scalić (format nieobsługiwany) — zostaje lokalny.
    NotMergeable {
        /// Dokument.
        name: String,
    },
    /// Magazyn sekretów niedostępny — strażnik działa tylko na wzorcach.
    SecretStoreUnavailable,
    /// Zredagowano ciągi wyglądające na sekrety.
    Redacted {
        /// Ścieżka wpisu.
        path: String,
        /// Liczba zredagowanych ciągów.
        count: u64,
    },
    /// Snapshot niezaszyfrowany (brak klucza maszyny).
    SnapshotUnencrypted,
}

/// Krok migracji (upcaster) zastosowany przy odczycie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpcastStep {
    /// Czego dotyczy (`manifest`, `session`, `turn`).
    pub entity: String,
    /// Wersja źródłowa.
    pub from: String,
    /// Wersja docelowa.
    pub to: String,
    /// Liczba rekordów.
    pub count: u64,
}

/// Wynik dry-run: co zostanie dodane/scalone/zastąpione, kolizje, ostrzeżenia, migracje.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DryRunReport {
    /// Elementy.
    pub items: Vec<ItemDiff>,
    /// Ostrzeżenia.
    pub warnings: Vec<Warning>,
    /// Migracje schematu.
    pub migrations: Vec<UpcastStep>,
    /// Mapa identyfikatorów sesji importowanych jako kopie (`paczka → nowy`).
    pub id_map: BTreeMap<SessionId, SessionId>,
}

impl DryRunReport {
    /// Liczba elementów, które zmienią stan lokalny.
    pub fn writes(&self) -> usize {
        self.items.iter().filter(|i| i.action.writes()).count()
    }

    /// Kolizje identyfikatorów sesji.
    pub fn collisions(&self) -> Vec<&ItemDiff> {
        self.items
            .iter()
            .filter(|i| i.state == ItemState::Collision)
            .collect()
    }
}

/// Podgląd paczki: manifest + dry-run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Inspection {
    /// Manifest (po migracji do bieżącej wersji schematu).
    pub manifest: Manifest,
    /// Dry-run.
    pub report: DryRunReport,
}

/// Wynik elementu importu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// Wykonano czynność.
    Done {
        /// Czynność.
        action: PlannedAction,
    },
    /// Nie powiodło się — element nietknięty (albo przywrócony ze snapshotu).
    Failed {
        /// Powód.
        reason: String,
    },
}

/// Wynik importu elementu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ItemOutcome {
    /// Element.
    pub item: ItemRef,
    /// Wynik.
    pub outcome: Outcome,
}

/// Identyfikator snapshotu (nazwa pliku bez rozszerzenia; `[0-9A-Za-z_-]`).
pub type SnapshotId = String;

/// Wynik importu.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImportReport {
    /// Snapshot wykonany przed importem (rollback jednym kliknięciem).
    pub snapshot: Option<SnapshotId>,
    /// Wyniki per element (bez elementów identycznych i pominiętych w planie).
    pub items: Vec<ItemOutcome>,
    /// Dodane.
    pub added: u64,
    /// Scalone.
    pub merged: u64,
    /// Zastąpione.
    pub replaced: u64,
    /// Zaimportowane jako kopie.
    pub copied: u64,
    /// Pominięte (plan) albo identyczne.
    pub skipped: u64,
    /// Nieudane.
    pub failed: u64,
    /// Mapa identyfikatorów kopii.
    pub id_map: BTreeMap<SessionId, SessionId>,
    /// Ostrzeżenia.
    pub warnings: Vec<Warning>,
}

/// Wynik eksportu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportReport {
    /// Plik paczki.
    pub path: PathBuf,
    /// Rozmiar pliku.
    pub file_bytes: u64,
    /// Manifest.
    pub manifest: Manifest,
    /// Ostrzeżenia (w tym redakcje sekretów i pominięte sesje prywatne).
    pub warnings: Vec<Warning>,
}

/// Snapshot na liście.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotInfo {
    /// Identyfikator.
    pub id: SnapshotId,
    /// Utworzenie.
    pub created_at: DateTime<Utc>,
    /// Liczba elementów do przywrócenia.
    pub items: u64,
}

/// Wynik rollbacku.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RollbackReport {
    /// Przywrócone elementy.
    pub restored: u64,
    /// Usunięte elementy utworzone przez import.
    pub removed: u64,
    /// Nieudane.
    pub failed: Vec<ItemOutcome>,
}

/// Wynik kopii zapasowej.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BackupReport {
    /// Eksport.
    pub export: ExportReport,
    /// Usunięte stare kopie (rotacja).
    pub rotated_out: Vec<PathBuf>,
}
