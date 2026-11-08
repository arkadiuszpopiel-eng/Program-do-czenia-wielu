//! `PersonaService` — implementacja kontraktu `Persona`.

use std::collections::BTreeMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use voice_persona_contract::{
    ChunkerCfg, EngineStyleTable, Lexicon, Origin, Persona, PersonaError, PersonaId, SpeechChunker,
    SpokenPlan, SpokenSentence, StylePlanner, StyleTags, VoiceBible,
};

use crate::chunker::SentenceChunker;
use crate::markdown::{apply_marks, split};
use crate::normalize::normalize;
use crate::style::TablePlanner;

/// Wbudowany słownik wymowy (skrótowce i nazwy własne częste w pracy z komputerem).
pub fn builtin_lexicon() -> Lexicon {
    let mut lx = Lexicon::new();
    let entries = [
        ("GitHub", "gitchab"),
        ("Svelte", "swelt"),
        ("WebView", "łebwiu"),
        ("Windows", "łindołs"),
        ("YouTube", "jutub"),
        ("iPhone", "ajfon"),
        ("Wi-Fi", "łifi"),
        ("e-mail", "imejl"),
        ("API", "a pe i"),
        ("URL", "u er el"),
        ("PDF", "pe de ef"),
        ("USB", "u es be"),
        ("CPU", "ce pe u"),
        ("LLM", "el el em"),
        ("TTS", "te te es"),
        ("STT", "es te te"),
        ("Claude", "klod"),
    ];
    for (word, pron) in entries {
        // Wpisy stałe przechodzą walidację (test jednostkowy); błąd = wpis pominięty.
        let _ = lx.insert(word, pron, Origin::Builtin);
    }
    lx
}

/// Serwis persony: biblie, słownik (edytowalny), normalizator, chunker, planista stylu.
#[derive(Debug)]
pub struct PersonaService {
    bibles: BTreeMap<PersonaId, VoiceBible>,
    lexicon: RwLock<Lexicon>,
    planner: TablePlanner,
}

impl Default for PersonaService {
    fn default() -> Self {
        Self::new()
    }
}

impl PersonaService {
    /// Cztery persony wbudowane + słownik wbudowany.
    pub fn new() -> Self {
        let bibles = VoiceBible::builtin_all()
            .into_iter()
            .map(|b| (b.persona.clone(), b))
            .collect();
        Self {
            bibles,
            lexicon: RwLock::new(builtin_lexicon()),
            planner: TablePlanner,
        }
    }

    /// Serwis z własnymi bibliami (każda walidowana) i pustym słownikiem.
    pub fn with_bibles(bibles: Vec<VoiceBible>) -> Result<Self, PersonaError> {
        let mut map = BTreeMap::new();
        for bible in bibles {
            bible.validate()?;
            map.insert(bible.persona.clone(), bible);
        }
        Ok(Self {
            bibles: map,
            lexicon: RwLock::new(Lexicon::new()),
            planner: TablePlanner,
        })
    }

    /// Zastępuje słownik (np. wczytany z `personas/lexicon.toml`).
    pub fn with_lexicon(self, lexicon: Lexicon) -> Self {
        Self {
            lexicon: RwLock::new(lexicon),
            ..self
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, Lexicon> {
        self.lexicon.read().unwrap_or_else(|p| p.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Lexicon> {
        self.lexicon.write().unwrap_or_else(|p| p.into_inner())
    }
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_speakable(text: &str) -> bool {
    text.chars().any(char::is_alphanumeric)
}

impl Persona for PersonaService {
    fn bible(&self, persona: &PersonaId) -> Result<VoiceBible, PersonaError> {
        self.bibles
            .get(persona)
            .cloned()
            .ok_or_else(|| PersonaError::UnknownPersona {
                id: persona.to_string(),
            })
    }

    fn normalize_pl(&self, text: &str) -> String {
        normalize(text, &self.read())
    }

    fn plan(
        &self,
        persona: &PersonaId,
        assistant_text: &str,
        engine: &EngineStyleTable,
    ) -> Result<SpokenPlan, PersonaError> {
        let bible = self.bible(persona)?;
        let parts = split(assistant_text);
        let mut chunker = SentenceChunker::new(ChunkerCfg::default());
        let mut chunks = chunker.push(&parts.spoken);
        chunks.extend(chunker.finish());
        let lexicon = self.read();
        let mut tags = StyleTags::default();
        let mut unsupported = Vec::new();
        let mut sentences = Vec::new();
        for chunk in chunks {
            let raw = apply_marks(&chunk.text, &parts.marks, &mut tags, &mut unsupported);
            let text = collapse(&normalize(&raw, &lexicon));
            if !is_speakable(&text) {
                continue;
            }
            let plan = self.planner.plan_style(&bible, &tags, engine);
            for item in plan.unsupported {
                if !unsupported.contains(&item) {
                    unsupported.push(item);
                }
            }
            sentences.push(SpokenSentence {
                text,
                tags,
                style: plan.style,
            });
        }
        Ok(SpokenPlan {
            persona: persona.clone(),
            sentences,
            on_screen: parts.screen,
            unsupported,
        })
    }

    fn chunker(&self, cfg: ChunkerCfg) -> Box<dyn SpeechChunker> {
        Box::new(SentenceChunker::new(cfg))
    }

    fn lexicon(&self) -> Lexicon {
        self.read().clone()
    }

    fn set_lexicon_entry(
        &self,
        word: &str,
        pron: &str,
        origin: Origin,
    ) -> Result<(), PersonaError> {
        self.write().insert(word, pron, origin).map(|_| ())
    }

    fn remove_lexicon_entry(&self, word: &str) -> Result<(), PersonaError> {
        self.write().remove(word).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_lexicon_is_complete_and_idempotent() {
        let lx = builtin_lexicon();
        assert_eq!(lx.len(), 17);
        for (_, entry) in lx.iter() {
            // Wymowa nie zawiera żadnego klucza (idempotencja normalizatora).
            let again = normalize(&entry.pron, &lx);
            assert_eq!(again, entry.pron);
        }
    }

    #[test]
    fn with_bibles_validates() {
        let mut bad = VoiceBible::builtin(&PersonaId::alfa()).unwrap();
        bad.perceived_age = 30;
        assert!(PersonaService::with_bibles(vec![bad]).is_err());
        let ok =
            PersonaService::with_bibles(vec![VoiceBible::builtin(&PersonaId::beta()).unwrap()])
                .unwrap();
        assert!(ok.bible(&PersonaId::alfa()).is_err());
        assert!(ok.lexicon().is_empty());
    }
}
