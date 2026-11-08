//! Kontrakt modułu `voice-persona` (docs/PLAN.md §6.6–6.7, docs/VOICE.md §12,
//! docs/modules/voice-persona/SPEC.md).
//!
//! Zawiera: tożsamość persony (`PersonaId` — re-eksport z `personas-contract`, walidacja
//! [`parse_persona_id`]), biblię głosu (`VoiceBible`), edytowalny słownik
//! wymowy (`Lexicon`), typy chunkera strumienia TTS (`Chunk`, `ChunkerCfg`), typy planisty stylu
//! (`StyleTags` → `SpeechStyle` przez tabelę silnika `EngineStyleTable`), plan mówienia
//! (`SpokenPlan`: kanał mówiony + ekranowy) oraz traity `Persona`, `TextNormalizer`,
//! `SpeechChunker`, `StylePlanner`. Implementacja: `voice-persona-impl`; atrapa: `voice-persona-fake`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod bible;
mod chunk;
mod error;
mod lexicon;
mod persona_id;
mod plan;
mod style;
mod traits;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use bible::{
    Consent, FORBIDDEN_PROMPT_WORDS, MAX_AGE, MIN_AGE, Provenance, Register, VoiceBible,
};
pub use chunk::{Boundary, Chunk, ChunkerCfg};
pub use error::PersonaError;
pub use lexicon::{Lexicon, LexiconEntry, MAX_PRON_CHARS, MAX_WORD_CHARS, Origin};
pub use persona_id::{PersonaId, parse_persona_id};
pub use plan::{CODE_ON_SCREEN, SpokenPlan, SpokenSentence};
pub use style::{
    Emotion, EmotionTag, Energy, EngineKind, EngineStyleTable, ParamRange, SpeechStyle, StylePlan,
    StyleTags, Tempo,
};
pub use traits::{Persona, SpeechChunker, StylePlanner, TextNormalizer};

use core_bus_contract::EventKind;

/// Zdarzenie diagnostyczne: utworzono plan mówienia (`plan()`).
pub const EVENT_PLAN_CREATED: &str = "voice.persona.plan_created";
/// Zmiana słownika wymowy (pierścień R0; zmiany Ulepszacza trafiają do Audytu).
pub const EVENT_LEXICON_CHANGED: &str = "voice.persona.lexicon_changed";
/// Silnik nie obsługuje żądanego znacznika stylu (degradacja, nigdy tekst tagu w mowie).
pub const EVENT_STYLE_UNSUPPORTED: &str = "voice.persona.style_unsupported";

/// Rodzaj zdarzenia magistrali dla nazwy z tego modułu.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_names_follow_convention() {
        for name in [
            EVENT_PLAN_CREATED,
            EVENT_LEXICON_CHANGED,
            EVENT_STYLE_UNSUPPORTED,
        ] {
            assert!(name.starts_with("voice.persona."));
            assert_eq!(event_kind(name).to_string(), name);
        }
    }

    #[test]
    fn public_types_have_json_schema() {
        let schema = schemars::schema_for!(SpokenPlan);
        let text = serde_json::to_string(&schema).unwrap();
        assert!(text.contains("sentences"));
        let schema = schemars::schema_for!(Lexicon);
        assert!(serde_json::to_string(&schema).unwrap().contains("pron"));
    }
}
