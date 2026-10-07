//! Atrapa modułu `voice-persona` (SPEC „Fake”): biblie z fixture'ów (wbudowane z PERSONAS.md),
//! normalizator tabelowy (słownik + cyfry czytane pojedynczo), plan bez stylów (styl neutralny),
//! chunker dzielący tylko na końcach linii. Deterministyczna; rejestruje wywołania `plan`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Mutex, MutexGuard};

use voice_persona_contract::{
    Boundary, CODE_ON_SCREEN, Chunk, ChunkerCfg, EngineStyleTable, Lexicon, Origin, Persona,
    PersonaError, PersonaId, SpeechChunker, SpeechStyle, SpokenPlan, SpokenSentence, StyleTags,
    VoiceBible,
};

const DIGIT_WORDS: [&str; 10] = [
    "zero",
    "jeden",
    "dwa",
    "trzy",
    "cztery",
    "pięć",
    "sześć",
    "siedem",
    "osiem",
    "dziewięć",
];

#[derive(Debug, Default)]
struct State {
    lexicon: Lexicon,
    planned: Vec<(PersonaId, String)>,
}

/// Deterministyczna atrapa `Persona`.
#[derive(Debug, Default)]
pub struct FakePersona {
    state: Mutex<State>,
}

impl FakePersona {
    /// Nowa atrapa z pustym słownikiem.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wszystkie wywołania `plan` (persona, tekst) w kolejności.
    pub fn planned(&self) -> Vec<(PersonaId, String)> {
        self.lock().planned.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Normalizacja tabelowa: wpisy słownika (pojedyncze słowa) i cyfry czytane pojedynczo.
pub fn table_normalize(text: &str, lexicon: &Lexicon) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if word.is_empty() {
            return;
        }
        let w = std::mem::take(word);
        match lexicon.get(&w) {
            Some(e) => out.push_str(&e.pron),
            None => out.push_str(&w),
        }
    };
    for c in text.chars() {
        if let Some(d) = c.to_digit(10) {
            flush(&mut word, &mut out);
            if out.chars().last().is_some_and(char::is_alphanumeric) {
                out.push(' ');
            }
            out.push_str(DIGIT_WORDS[usize::try_from(d).unwrap_or(0)]);
            continue;
        }
        if c.is_alphabetic() {
            if word.is_empty()
                && out
                    .chars()
                    .last()
                    .is_some_and(|p| p.is_ascii_alphanumeric() && !p.is_alphabetic())
            {
                out.push(' ');
            }
            word.push(c);
            continue;
        }
        flush(&mut word, &mut out);
        out.push(c);
    }
    flush(&mut word, &mut out);
    out
}

fn strip_line(line: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in line.trim_start_matches(['#', '>', '-', '*', ' ']).chars() {
        match c {
            '[' => in_tag = true,
            ']' => in_tag = false,
            '*' | '_' | '`' | '~' if !in_tag => {}
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Chunker atrapy: fragment = linia.
#[derive(Debug, Default)]
pub struct LineChunker {
    buf: String,
}

impl SpeechChunker for LineChunker {
    fn push(&mut self, delta: &str) -> Vec<Chunk> {
        self.buf.push_str(delta);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.find('\n') {
            let line: String = self.buf.drain(..=pos).collect();
            if !line.trim().is_empty() {
                out.push(Chunk {
                    text: line.trim().to_owned(),
                    boundary: Boundary::Line,
                });
            }
        }
        out
    }

    fn finish(&mut self) -> Vec<Chunk> {
        let rest = std::mem::take(&mut self.buf);
        if rest.trim().is_empty() {
            return Vec::new();
        }
        vec![Chunk {
            text: rest.trim().to_owned(),
            boundary: Boundary::End,
        }]
    }
}

impl Persona for FakePersona {
    fn bible(&self, persona: &PersonaId) -> Result<VoiceBible, PersonaError> {
        VoiceBible::builtin(persona).ok_or_else(|| PersonaError::UnknownPersona {
            id: persona.to_string(),
        })
    }

    fn normalize_pl(&self, text: &str) -> String {
        table_normalize(text, &self.lock().lexicon)
    }

    fn plan(
        &self,
        persona: &PersonaId,
        assistant_text: &str,
        engine: &EngineStyleTable,
    ) -> Result<SpokenPlan, PersonaError> {
        self.bible(persona)?;
        let mut st = self.lock();
        st.planned
            .push((persona.clone(), assistant_text.to_owned()));
        let mut sentences = Vec::new();
        let mut on_screen = Vec::new();
        let mut lines = assistant_text.lines();
        let push = |text: String, sentences: &mut Vec<SpokenSentence>| {
            sentences.push(SpokenSentence {
                text,
                tags: StyleTags::default(),
                style: SpeechStyle::neutral(engine.engine),
            });
        };
        while let Some(line) = lines.next() {
            if line.trim_start().starts_with("```") {
                let mut block = vec![line];
                for inner in lines.by_ref() {
                    block.push(inner);
                    if inner.trim_start().starts_with("```") {
                        break;
                    }
                }
                on_screen.push(block.join("\n"));
                push(CODE_ON_SCREEN.to_owned(), &mut sentences);
            } else if line.trim_start().starts_with('|') {
                on_screen.push(line.to_owned());
            } else {
                let text = table_normalize(&strip_line(line), &st.lexicon);
                if text.chars().any(char::is_alphanumeric) {
                    push(text, &mut sentences);
                }
            }
        }
        Ok(SpokenPlan {
            persona: persona.clone(),
            sentences,
            on_screen,
            unsupported: Vec::new(),
        })
    }

    fn chunker(&self, _cfg: ChunkerCfg) -> Box<dyn SpeechChunker> {
        Box::new(LineChunker::default())
    }

    fn lexicon(&self) -> Lexicon {
        self.lock().lexicon.clone()
    }

    fn set_lexicon_entry(
        &self,
        word: &str,
        pron: &str,
        origin: Origin,
    ) -> Result<(), PersonaError> {
        self.lock().lexicon.insert(word, pron, origin).map(|_| ())
    }

    fn remove_lexicon_entry(&self, word: &str) -> Result<(), PersonaError> {
        self.lock().lexicon.remove(word).map(|_| ())
    }
}
