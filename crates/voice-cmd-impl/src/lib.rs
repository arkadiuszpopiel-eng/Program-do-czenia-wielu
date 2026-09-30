//! Implementacja modułu `voice-cmd` (docs/modules/voice-cmd/SPEC.md): szybka ścieżka komend
//! głosowych bez LLM. Gramatyka PL/EN (`Grammar`) jest kompilowana do wzorców; wypowiedź jest
//! komendą tylko wtedy, gdy składa się wyłącznie z fraz komend, wypełniaczy i imion person —
//! dzięki temu zdania z tymi słowami w innym znaczeniu („pauza w szkole była długa”) nie są
//! komendami. Tolerancja szumu ASR: `fold` (bez polskich znaków) i odległość edycyjna.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod fuzzy;
mod pattern;
mod recognizer;

pub use fuzzy::{EXACT, ONE_EDIT, TWO_EDITS, levenshtein, word_score};
pub use recognizer::GrammarRecognizer;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
