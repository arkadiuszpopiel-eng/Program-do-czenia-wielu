//! Błędy modułu `voice-persona`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Błędy kontraktu `Persona`.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum PersonaError {
    /// Nieznana persona albo niepoprawny identyfikator.
    #[error("nieznana persona: `{id}`")]
    UnknownPersona {
        /// Identyfikator.
        id: String,
    },
    /// Wpis słownika wymowy odrzucony przez walidację.
    #[error("niepoprawny wpis słownika `{word}`: {reason}")]
    InvalidLexiconEntry {
        /// Słowo (klucz).
        word: String,
        /// Powód odrzucenia.
        reason: String,
    },
    /// Brak słowa w słowniku (np. przy usuwaniu).
    #[error("słowa `{word}` nie ma w słowniku wymowy")]
    NotInLexicon {
        /// Słowo.
        word: String,
    },
    /// Biblia głosu narusza niezmienniki (wiek, zakazane słowa promptu, zgoda).
    #[error("niepoprawna biblia głosu: {reason}")]
    InvalidBible {
        /// Powód.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_tagged() {
        let json = serde_json::to_value(PersonaError::NotInLexicon { word: "x".into() }).unwrap();
        assert_eq!(json["error"], "not_in_lexicon");
    }
}
