//! Plan mówienia: kanał mówiony (zdania znormalizowane + styl) i kanał ekranowy (kod, tabele).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{PersonaId, SpeechStyle, StyleTags};

/// Tekst wstawiany w kanale mówionym w miejsce kodu (kod trafia na ekran).
pub const CODE_ON_SCREEN: &str = "(kod na ekranie)";

/// Zdanie kanału mówionego.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpokenSentence {
    /// Tekst po normalizacji (bez markdown, cyfr i znaczników).
    pub text: String,
    /// Znaczniki obowiązujące dla zdania.
    pub tags: StyleTags,
    /// Parametry silnika.
    pub style: SpeechStyle,
}

/// Wynik `Persona::plan`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpokenPlan {
    /// Persona mówiąca.
    pub persona: PersonaId,
    /// Kanał mówiony — zdania w kolejności.
    pub sentences: Vec<SpokenSentence>,
    /// Kanał ekranowy — bloki kodu i tabele w oryginale.
    pub on_screen: Vec<String>,
    /// Znaczniki stylu, których silnik nie obsłużył lub których nie rozpoznano.
    pub unsupported: Vec<String>,
}

impl SpokenPlan {
    /// Cały kanał mówiony jako jeden tekst (zdania rozdzielone spacją).
    pub fn spoken_text(&self) -> String {
        self.sentences
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}
