//! Typy chunkera strumienia tekstu do TTS (zdanie po zdaniu, PLAN §6.7).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parametry chunkera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChunkerCfg {
    /// Zdanie dłuższe niż tyle znaków jest dzielone na przecinku.
    pub max_chars: usize,
    /// Limit dla pierwszego fragmentu (krótszy → szybszy pierwszy dźwięk).
    pub first_max_chars: usize,
    /// Twardy limit: bez przecinka dzielimy na ostatniej spacji.
    pub hard_max_chars: usize,
    /// Minimalna długość fragmentu przy podziale na przecinku.
    pub min_clause_chars: usize,
}

impl Default for ChunkerCfg {
    fn default() -> Self {
        Self {
            max_chars: 160,
            first_max_chars: 90,
            hard_max_chars: 300,
            min_clause_chars: 20,
        }
    }
}

/// Dlaczego fragment się skończył.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Boundary {
    /// Koniec zdania (`.`, `!`, `?`, `…`).
    Sentence,
    /// Średnik.
    Semicolon,
    /// Koniec linii / akapitu / pozycji listy.
    Line,
    /// Przecinek w długim zdaniu.
    Clause,
    /// Wymuszony podział na spacji (brak przecinka w bardzo długim zdaniu).
    Forced,
    /// Blok kodu (```…```) jako całość.
    Code,
    /// Reszta tekstu przy zakończeniu strumienia.
    End,
}

/// Fragment do syntezy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Chunk {
    /// Tekst fragmentu (przycięty, niepusty).
    pub text: String,
    /// Rodzaj granicy.
    pub boundary: Boundary,
}
