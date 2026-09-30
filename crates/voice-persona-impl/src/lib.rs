//! Implementacja modułu `voice-persona` (docs/modules/voice-persona/SPEC.md): normalizator PL,
//! słownik wymowy z pierwszeństwem przed regułami, chunker strumienia TTS, planista stylu
//! (biblia głosu + znaczniki → parametry silnika wg tabel) i podział mówione/ekranowe.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod numbers;

mod chunker;
mod markdown;
mod normalize;
mod service;
mod style;

pub use chunker::SentenceChunker;
pub use markdown::TABLE_ON_SCREEN;
pub use normalize::{PlNormalizer, normalize};
pub use service::{PersonaService, builtin_lexicon};
pub use style::{FAST_FACTOR, SLOW_FACTOR, TablePlanner, builtin_tables, emotion_label, table_for};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
