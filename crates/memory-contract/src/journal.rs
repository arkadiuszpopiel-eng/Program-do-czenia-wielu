//! Dziennik zmian pamięci (konsolidacja, edycje, awanse) z cofaniem.
//!
//! Każda zmiana konsolidacji to [`ChangeOp`] w [`ChangeSet`] jednego zakresu; silnik zapisuje ją
//! atomowo razem z rekordem [`JournalRecord`] (migawki „przed” i „po”), więc da się ją cofnąć.
//! Wygaszenie (retencja) jest **nieodwracalne** — dziennik nie przechowuje treści wygasłych wpisów.
//! `forget` usuwa rekordy dziennika odwołujące się do zapomnianych wpisów (kaskada).

use std::fmt;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{EntryRef, SupersedeReason};
use crate::types::{MemoryEntry, MemoryId, MemoryScope, NewMemory};

/// Identyfikator zmiany w dzienniku.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ChangeId(pub String);

impl fmt::Display for ChangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Rodzaj zmiany.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Nowy wpis pochodny (fakt, streszczenie, umiejętność).
    Create,
    /// Nowa wersja zastępująca starszą (sprzeczność).
    Supersede,
    /// Duplikaty scalone z wpisem wiodącym.
    Merge,
    /// Wpis wygaszony (retencja/TTL) — nieodwracalne.
    Expire,
    /// Epizody oznaczone jako przetworzone.
    MarkConsolidated,
    /// Sprzeczność bez automatycznego rozstrzygnięcia (decyzja użytkownika).
    Conflict,
    /// Edycja w Inspektorze (nowa wersja).
    Edit,
    /// Awans do zakresu szerszego (kopia).
    Promote,
}

/// Rekord dziennika (w bazie zakresu, którego dotyczy).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct JournalRecord {
    /// Identyfikator.
    pub id: ChangeId,
    /// Przebieg konsolidacji (`None` = zmiana użytkownika).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Kiedy.
    pub at: DateTime<Utc>,
    /// Zakres.
    pub scope: MemoryScope,
    /// Rodzaj.
    pub kind: ChangeKind,
    /// Wszystkie wpisy, których dotyczy (kaskada `forget` usuwa rekord, gdy zniknie którykolwiek).
    pub refs: Vec<MemoryId>,
    /// Migawki wpisów przed zmianą (puste dla wygaszenia — nieodwracalne).
    pub before: Vec<MemoryEntry>,
    /// Migawki wpisów po zmianie.
    pub after: Vec<MemoryEntry>,
    /// Opis (bez treści wpisów — np. „scalono 3 duplikaty”).
    pub note: String,
    /// Czy cofnięto.
    #[serde(default)]
    pub undone: bool,
}

impl JournalRecord {
    /// Czy rekord dotyczy wpisu.
    pub fn touches(&self, id: &MemoryId) -> bool {
        self.refs.contains(id)
    }
}

/// Jedna zmiana konsolidacji.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ChangeOp {
    /// Nowy wpis pochodny (źródła w `entry.origin.derived_from`, ten sam zakres).
    Create {
        /// Wpis.
        entry: NewMemory,
        /// Zatwierdzony od razu (`auto_extract = on`) albo oczekujący (`ask`).
        approved: bool,
        /// Opis.
        note: String,
    },
    /// Nowa wersja faktu zastępująca `old` (sprzeczność).
    Supersede {
        /// Wpis zastępowany.
        old: MemoryId,
        /// Nowa wersja.
        entry: NewMemory,
        /// Powód.
        reason: SupersedeReason,
        /// Opis.
        note: String,
    },
    /// Istniejący nowszy fakt `by` zastępuje istniejący starszy `old` (sprzeczność rozstrzygnięta
    /// regułą: ten sam temat, zaufanie nowszego ≥ starszego).
    Resolve {
        /// Wpis starszy (zostaje w historii jako zastąpiony).
        old: MemoryId,
        /// Wpis nowszy.
        by: MemoryId,
        /// Opis.
        note: String,
    },
    /// Duplikaty scalone z wpisem wiodącym `keep` (duplikaty → zastąpione, przywracane, gdy
    /// wiodący zniknie przez `forget`).
    Merge {
        /// Wpis wiodący.
        keep: MemoryId,
        /// Duplikaty.
        duplicates: Vec<MemoryId>,
        /// Opis.
        note: String,
    },
    /// Wygaszenie (retencja): usunięcie bez kaskady i bez migawki — nieodwracalne.
    Expire {
        /// Wpis.
        id: MemoryId,
        /// Opis.
        note: String,
    },
    /// Oznaczenie epizodów jako przetworzonych.
    MarkConsolidated {
        /// Wpisy.
        ids: Vec<MemoryId>,
    },
    /// Sprzeczność do decyzji użytkownika (tylko dziennik).
    FlagConflict {
        /// Wpis starszy.
        a: MemoryId,
        /// Wpis nowszy.
        b: MemoryId,
        /// Opis.
        note: String,
    },
}

/// Zestaw zmian w jednym zakresie (atomowo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeSet {
    /// Zakres.
    pub scope: MemoryScope,
    /// Przebieg konsolidacji.
    pub run: String,
    /// Zmiany (w kolejności).
    pub ops: Vec<ChangeOp>,
}

/// Wynik zastosowania zestawu zmian.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeReport {
    /// Rekordy dziennika (po jednym na zmianę).
    pub changes: Vec<ChangeId>,
    /// Nowe wpisy.
    pub created: Vec<EntryRef>,
    /// Wpisy zastąpione (sprzeczność, scalenie).
    pub superseded: Vec<EntryRef>,
    /// Wpisy wygaszone.
    pub expired: Vec<EntryRef>,
    /// Zgłoszone konflikty.
    pub conflicts: usize,
}

/// Wynik cofnięcia zmiany.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UndoReport {
    /// Cofnięta zmiana.
    pub change: Option<ChangeId>,
    /// Usunięte wpisy (utworzone przez zmianę) wraz z kaskadą.
    pub removed: Vec<EntryRef>,
    /// Przywrócone migawki.
    pub restored: Vec<EntryRef>,
    /// Pominięte (wpis zniknął lub zmienił się od tamtej pory).
    pub skipped: Vec<EntryRef>,
}
