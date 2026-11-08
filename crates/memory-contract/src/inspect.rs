//! Inspektor pamięci (API dla UI, PLAN §10, makieta 9): lista, filtry, wyszukiwanie,
//! „dlaczego to pamiętam”, historia wersji, eksport.

use chrono::{DateTime, Utc};
use core_bus_contract::SessionId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::journal::JournalRecord;
use crate::model::{EntryRef, EntryState, entry_state};
use crate::types::{Layer, MemoryEntry, MemoryScope, Provenance};

/// Domyślny rozmiar strony Inspektora.
pub const DEFAULT_PAGE: usize = 50;
/// Maksymalny rozmiar strony.
pub const MAX_PAGE: usize = 500;

/// Zapytanie Inspektora (wszystkie filtry łączone AND; puste listy = bez filtra).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InspectorQuery {
    /// Zakresy (puste = wszystkie z danymi).
    #[serde(default)]
    pub scopes: Vec<MemoryScope>,
    /// Wyszukiwanie (hybryda; puste = lista po dacie malejąco).
    #[serde(default)]
    pub text: Option<String>,
    /// Warstwy.
    #[serde(default)]
    pub layers: Vec<Layer>,
    /// Stany (puste = wszystkie).
    #[serde(default)]
    pub states: Vec<EntryState>,
    /// Tylko zaufane (`Some(true)`) / tylko niezaufane (`Some(false)`).
    #[serde(default)]
    pub trusted: Option<bool>,
    /// Tylko przypięte / nieprzypięte.
    #[serde(default)]
    pub pinned: Option<bool>,
    /// Sesja źródłowa (`origin.session` albo zakres sesji).
    #[serde(default)]
    pub session: Option<SessionId>,
    /// Źródło treści niezaufanej/importu (fragment, bez wielkości liter).
    #[serde(default)]
    pub source: Option<String>,
    /// Utworzone od.
    #[serde(default)]
    pub created_after: Option<DateTime<Utc>>,
    /// Utworzone przed.
    #[serde(default)]
    pub created_before: Option<DateTime<Utc>>,
    /// Pominięte pozycje (stronicowanie).
    #[serde(default)]
    pub offset: usize,
    /// Rozmiar strony (0 → [`DEFAULT_PAGE`], max [`MAX_PAGE`]).
    #[serde(default)]
    pub limit: usize,
}

impl InspectorQuery {
    /// Rozmiar strony po normalizacji.
    pub fn page_size(&self) -> usize {
        match self.limit {
            0 => DEFAULT_PAGE,
            n => n.min(MAX_PAGE),
        }
    }

    /// Czy wpis przechodzi przez filtry (bez tekstu i zakresów — te liczy silnik).
    pub fn accepts(&self, entry: &MemoryEntry, now: DateTime<Utc>) -> bool {
        let source_of = |p: &Provenance| match p {
            Provenance::UntrustedContent { source } | Provenance::Import { source } => {
                Some(source.to_lowercase())
            }
            _ => None,
        };
        (self.layers.is_empty() || self.layers.contains(&entry.layer))
            && (self.states.is_empty() || self.states.contains(&entry_state(entry, now)))
            && self.trusted.is_none_or(|t| t == entry.trusted)
            && self.pinned.is_none_or(|p| p == entry.pinned)
            && self.session.as_ref().is_none_or(|s| {
                entry.origin.session.as_ref() == Some(s)
                    || entry.scope == MemoryScope::Session(s.clone())
            })
            && self.source.as_ref().is_none_or(|needle| {
                source_of(&entry.provenance).is_some_and(|src| src.contains(&needle.to_lowercase()))
            })
            && self.created_after.is_none_or(|t| entry.created_at >= t)
            && self.created_before.is_none_or(|t| entry.created_at < t)
    }
}

/// Pozycja listy Inspektora.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InspectorItem {
    /// Wpis (z pełną proweniencją).
    pub entry: MemoryEntry,
    /// Stan.
    pub state: EntryState,
    /// Wynik wyszukiwania (gdy zapytanie miało tekst).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

/// Strona wyników.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InspectorPage {
    /// Pozycje.
    pub items: Vec<InspectorItem>,
    /// Liczba wszystkich pasujących (przed stronicowaniem).
    pub total: usize,
}

/// Źródło wpisu w wyjaśnieniu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SourceLink {
    /// Odwołanie.
    pub entry: EntryRef,
    /// Czy źródło nadal istnieje (`false` → wygasło lub zapomniano).
    pub exists: bool,
    /// Stan źródła, gdy istnieje.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<EntryState>,
}

/// „Dlaczego to pamiętam”: proweniencja, wyprowadzenie, wersje, dziennik i powody po polsku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Explanation {
    /// Wpis.
    pub entry: MemoryEntry,
    /// Stan.
    pub state: EntryState,
    /// Powody (zdania dla UI; bez treści innych wpisów).
    pub reasons: Vec<String>,
    /// Źródła (z `origin.derived_from`).
    pub sources: Vec<SourceLink>,
    /// Historia wersji od najstarszej do najnowszej (łącznie z tym wpisem).
    pub versions: Vec<MemoryEntry>,
    /// Duplikaty scalone z tym wpisem.
    pub merged: Vec<EntryRef>,
    /// Wpisy wyprowadzone z tego wpisu (kopie, streszczenia, fakty).
    pub derived: Vec<EntryRef>,
    /// Zmiany w dzienniku dotyczące wpisu (malejąco po czasie).
    pub journal: Vec<JournalRecord>,
    /// Wygaśnięcie (TTL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

/// Edycja wpisu w Inspektorze (nowa wersja; pola `None` bez zmian).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EntryEdit {
    /// Nowa treść.
    #[serde(default)]
    pub text: Option<String>,
    /// Nowy temat (`Some(None)` usuwa temat).
    #[serde(default)]
    pub subject: Option<Option<String>>,
    /// Nowe encje.
    #[serde(default)]
    pub entities: Option<Vec<String>>,
    /// Nowa pewność.
    #[serde(default)]
    pub confidence: Option<f32>,
    /// Nowy TTL (`Some(None)` = bez wygasania).
    #[serde(default)]
    pub ttl_secs: Option<Option<u64>>,
}

impl EntryEdit {
    /// Czy edycja cokolwiek zmienia.
    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.subject.is_none()
            && self.entities.is_none()
            && self.confidence.is_none()
            && self.ttl_secs.is_none()
    }
}
