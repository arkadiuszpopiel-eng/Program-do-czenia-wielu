//! Traity modułu `voice-persona`.

use crate::{
    Chunk, ChunkerCfg, EngineStyleTable, Lexicon, Origin, PersonaError, PersonaId, SpokenPlan,
    StylePlan, StyleTags, VoiceBible,
};

/// Normalizator tekstu do mowy (pl-PL): liczby, daty, godziny, waluty, jednostki, skróty,
/// URL/e-mail, kod. Deterministyczny; słownik wymowy ma pierwszeństwo przed regułami.
pub trait TextNormalizer: Send + Sync {
    /// Normalizuje tekst z użyciem słownika.
    fn normalize(&self, text: &str, lexicon: &Lexicon) -> String;
}

/// Chunker strumienia tekstu (LLM → TTS). Stanowy: `push` przyjmuje kolejne kawałki strumienia
/// i zwraca fragmenty gotowe do syntezy; `finish` opróżnia resztę.
pub trait SpeechChunker: Send {
    /// Dokłada kawałek strumienia; zwraca zamknięte fragmenty.
    fn push(&mut self, delta: &str) -> Vec<Chunk>;
    /// Kończy strumień; zwraca pozostałe fragmenty.
    fn finish(&mut self) -> Vec<Chunk>;
}

/// Planista stylu: biblia głosu + znaczniki → parametry silnika wg tabeli silnika.
pub trait StylePlanner: Send + Sync {
    /// Wyznacza styl dla zdania.
    fn plan_style(
        &self,
        bible: &VoiceBible,
        tags: &StyleTags,
        table: &EngineStyleTable,
    ) -> StylePlan;
}

/// Kontrakt modułu `voice-persona` (SPEC).
pub trait Persona: Send + Sync {
    /// Biblia głosu persony.
    fn bible(&self, persona: &PersonaId) -> Result<VoiceBible, PersonaError>;
    /// Normalizacja PL z bieżącym słownikiem wymowy.
    fn normalize_pl(&self, text: &str) -> String;
    /// Plan mówienia: podział mówione/ekranowe, zdania, normalizacja, styl → silnik.
    fn plan(
        &self,
        persona: &PersonaId,
        assistant_text: &str,
        engine: &EngineStyleTable,
    ) -> Result<SpokenPlan, PersonaError>;
    /// Nowy chunker strumienia.
    fn chunker(&self, cfg: ChunkerCfg) -> Box<dyn SpeechChunker>;
    /// Kopia słownika wymowy.
    fn lexicon(&self) -> Lexicon;
    /// Dodaje / zmienia wpis słownika (walidacja jak `Lexicon::insert`).
    fn set_lexicon_entry(&self, word: &str, pron: &str, origin: Origin)
    -> Result<(), PersonaError>;
    /// Usuwa wpis słownika.
    fn remove_lexicon_entry(&self, word: &str) -> Result<(), PersonaError>;
}
