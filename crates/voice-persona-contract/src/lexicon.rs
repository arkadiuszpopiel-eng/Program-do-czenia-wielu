//! Słownik wymowy (edytowalny; pierścień R0 Ulepszacza, docs/VOICE.md §12).
//!
//! Klucz = słowo lub fraza dopasowywana jako całe słowo, bez rozróżniania wielkości liter.
//! Wpis ma pierwszeństwo przed regułami normalizatora. Wymowa nie może zawierać cyfr ani
//! znaczników (`[`, `]`, `` ` ``), żeby wynik normalizacji był zawsze „mówialny”.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::PersonaError;

/// Maksymalna długość słowa (klucza) w znakach.
pub const MAX_WORD_CHARS: usize = 64;
/// Maksymalna długość wymowy w znakach.
pub const MAX_PRON_CHARS: usize = 128;

/// Kto dodał wpis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Wpis wbudowany (domyślny słownik).
    Builtin,
    /// Użytkownik (Ustawienia → Głos → Słownik wymowy) — działa natychmiast.
    User,
    /// Ulepszacz (po bramce ewaluacyjnej, cofalny; zmiana trafia do Audytu).
    Improver,
}

/// Wpis słownika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LexiconEntry {
    /// Słowo w pisowni oryginalnej (do wyświetlania w edytorze).
    pub word: String,
    /// Zapis fonetyczny / zastępczy czytany przez TTS.
    pub pron: String,
    /// Pochodzenie wpisu.
    pub origin: Origin,
}

/// Słownik wymowy: mapa `klucz (małe litery) → wpis`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Lexicon {
    entries: BTreeMap<String, LexiconEntry>,
}

impl Lexicon {
    /// Pusty słownik.
    pub fn new() -> Self {
        Self::default()
    }

    /// Klucz dopasowania: przycięte, małe litery.
    pub fn key_of(word: &str) -> String {
        word.trim().to_lowercase()
    }

    /// Waliduje wpis bez dodawania go.
    pub fn validate_entry(word: &str, pron: &str) -> Result<(), PersonaError> {
        let invalid = |reason: &str| PersonaError::InvalidLexiconEntry {
            word: word.to_owned(),
            reason: reason.to_owned(),
        };
        let word_t = word.trim();
        let pron_t = pron.trim();
        if word_t.is_empty() {
            return Err(invalid("puste słowo"));
        }
        if pron_t.is_empty() {
            return Err(invalid("pusta wymowa"));
        }
        if word_t.chars().count() > MAX_WORD_CHARS {
            return Err(invalid("słowo za długie"));
        }
        if pron_t.chars().count() > MAX_PRON_CHARS {
            return Err(invalid("wymowa za długa"));
        }
        if pron_t.chars().any(|c| c.is_ascii_digit()) {
            return Err(invalid("wymowa nie może zawierać cyfr"));
        }
        if pron_t
            .chars()
            .any(|c| matches!(c, '[' | ']' | '`' | '<' | '>'))
        {
            return Err(invalid("wymowa nie może zawierać znaczników"));
        }
        if pron_t.chars().any(char::is_control) || word_t.chars().any(char::is_control) {
            return Err(invalid("znaki sterujące są niedozwolone"));
        }
        Ok(())
    }

    /// Dodaje lub zastępuje wpis; zwraca poprzedni wpis dla tego klucza.
    pub fn insert(
        &mut self,
        word: &str,
        pron: &str,
        origin: Origin,
    ) -> Result<Option<LexiconEntry>, PersonaError> {
        Self::validate_entry(word, pron)?;
        let entry = LexiconEntry {
            word: word.trim().to_owned(),
            pron: pron.trim().to_owned(),
            origin,
        };
        Ok(self.entries.insert(Self::key_of(word), entry))
    }

    /// Usuwa wpis.
    pub fn remove(&mut self, word: &str) -> Result<LexiconEntry, PersonaError> {
        self.entries
            .remove(&Self::key_of(word))
            .ok_or_else(|| PersonaError::NotInLexicon {
                word: word.to_owned(),
            })
    }

    /// Wpis dla słowa (bez rozróżniania wielkości liter).
    pub fn get(&self, word: &str) -> Option<&LexiconEntry> {
        self.entries.get(&Self::key_of(word))
    }

    /// Pary `(klucz, wpis)` w kolejności klucza.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &LexiconEntry)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Liczba wpisów.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Czy słownik jest pusty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_remove_case_insensitive() {
        let mut lx = Lexicon::new();
        assert!(
            lx.insert("GitHub", "gitchab", Origin::User)
                .unwrap()
                .is_none()
        );
        assert_eq!(lx.get("github").unwrap().pron, "gitchab");
        let prev = lx.insert("GITHUB", "githab", Origin::Improver).unwrap();
        assert_eq!(prev.unwrap().pron, "gitchab");
        assert_eq!(lx.len(), 1);
        assert_eq!(lx.remove("GitHub").unwrap().pron, "githab");
        assert!(lx.is_empty());
        assert_eq!(
            lx.remove("x"),
            Err(PersonaError::NotInLexicon { word: "x".into() })
        );
    }

    #[test]
    fn rejects_invalid_entries() {
        let mut lx = Lexicon::new();
        assert!(lx.insert("", "a", Origin::User).is_err());
        assert!(lx.insert("a", " ", Origin::User).is_err());
        assert!(lx.insert("GPT", "dżi pi ti 5", Origin::User).is_err());
        assert!(lx.insert("x", "[emocja:radość]", Origin::User).is_err());
        assert!(lx.insert(&"x".repeat(65), "iks", Origin::User).is_err());
        assert!(lx.insert("x", &"a".repeat(129), Origin::User).is_err());
        assert!(lx.insert("x\u{7}", "a", Origin::User).is_err());
    }

    #[test]
    fn serde_round_trip() {
        let mut lx = Lexicon::new();
        lx.insert("Tauri", "tałri", Origin::User).unwrap();
        let json = serde_json::to_string(&lx).unwrap();
        assert!(json.contains("\"tauri\""));
        let back: Lexicon = serde_json::from_str(&json).unwrap();
        assert_eq!(back, lx);
        assert_eq!(back.iter().count(), 1);
    }
}
