//! Stan indeksu wektorowego bazy i postęp przebudowy po zmianie embeddera.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Stan wektorów w jednej bazie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VectorStatus {
    /// Wektory bieżącego embeddera. `missing` = dokumenty bez wektora (embedder niedostępny przy
    /// zapisie — dokument jest w FTS, wektor uzupełni krok przebudowy).
    Ready {
        /// Embedder indeksu (`model_id/wymiar`; pusty = baza bez indeksu).
        embedder: String,
        /// Dokumenty bez wektora.
        missing: u64,
    },
    /// Przebudowa po zmianie embeddera: stare wektory nie są używane, zapytania wektorowe
    /// i hybrydowe działają jak pełnotekstowe (FTS), aż przebudowa się skończy.
    Rebuilding {
        /// Embedder starych wektorów.
        from: String,
        /// Embedder docelowy (bieżący).
        to: String,
        /// Dokumenty już przeliczone.
        done: u64,
        /// Wszystkie dokumenty.
        total: u64,
    },
}

impl Default for VectorStatus {
    fn default() -> Self {
        VectorStatus::Ready {
            embedder: String::new(),
            missing: 0,
        }
    }
}

impl VectorStatus {
    /// Czy zapytania wektorowe korzystają z wektorów (inaczej spadają do FTS).
    pub fn vectors_usable(&self) -> bool {
        matches!(self, VectorStatus::Ready { .. })
    }

    /// Czy baza wymaga kroków przebudowy (zmiana embeddera albo brakujące wektory).
    pub fn needs_work(&self) -> bool {
        match self {
            VectorStatus::Ready { missing, .. } => *missing > 0,
            VectorStatus::Rebuilding { .. } => true,
        }
    }
}

/// Wynik jednego kroku przebudowy (`TxIndexer::reindex_step`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReindexProgress {
    /// Wektory policzone w tym kroku.
    pub embedded: u64,
    /// Dokumenty z wektorem bieżącego embeddera (po kroku).
    pub done: u64,
    /// Wszystkie dokumenty.
    pub total: u64,
    /// Czy baza jest gotowa (brak przebudowy i brakujących wektorów).
    pub finished: bool,
}

impl ReindexProgress {
    /// Nic do zrobienia (`total` dokumentów, wszystkie z wektorem).
    pub fn finished(total: u64) -> Self {
        Self {
            embedded: 0,
            done: total,
            total,
            finished: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_flags_and_serde() {
        let ready = VectorStatus::default();
        assert!(ready.vectors_usable() && !ready.needs_work());
        let missing = VectorStatus::Ready {
            embedder: "e/8".into(),
            missing: 2,
        };
        assert!(missing.vectors_usable() && missing.needs_work());
        let rebuilding = VectorStatus::Rebuilding {
            from: "a/8".into(),
            to: "b/16".into(),
            done: 1,
            total: 3,
        };
        assert!(!rebuilding.vectors_usable() && rebuilding.needs_work());
        let json = serde_json::to_value(&rebuilding).unwrap();
        assert_eq!(json["state"], "rebuilding");
        assert_eq!(
            serde_json::from_value::<VectorStatus>(json).unwrap(),
            rebuilding
        );
        assert_eq!(ReindexProgress::finished(5).done, 5);
        assert!(ReindexProgress::finished(0).finished);
    }
}
