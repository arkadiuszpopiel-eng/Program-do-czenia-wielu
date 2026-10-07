//! Implementacja modułu `voice-cmd` (docs/modules/voice-cmd/SPEC.md): szybka ścieżka komend
//! głosowych bez LLM. Gramatyka PL/EN (`Grammar`) jest kompilowana do wzorców; wypowiedź jest
//! komendą tylko wtedy, gdy składa się wyłącznie z fraz komend, wypełniaczy i imion person —
//! dzięki temu zdania z tymi słowami w innym znaczeniu („pauza w szkole była długa”) nie są
//! komendami. Tolerancja szumu ASR: `fold` (bez polskich znaków) i odległość edycyjna.
//!
//! Rdzeń (`GrammarRecognizer`, dopasowanie rozmyte) mieszka w `voice-cmd-contract` (deterministyczne
//! funkcje bez I/O, jak `DialogMachine`), żeby runnery ewaluacji F2 mogły go użyć bez zależności od
//! `-impl`; ten crate jest implementacją modułu (manifest, testy kontraktowe na zamrożonym zestawie).

pub use voice_cmd_contract::{
    EXACT, GrammarRecognizer, ONE_EDIT, TWO_EDITS, levenshtein, word_score,
};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
