//! Kompilacja fraz gramatyki do wzorców i dopasowanie od pozycji w wypowiedzi.

use crate::{CommandKind, Grammar, fold};
use personas_contract::PersonaId;

use super::fuzzy::word_score;

/// Element wzorca.
#[derive(Debug, Clone)]
enum Elem {
    /// Słowo z alternatywami (po `fold`), opcjonalne lub nie.
    Word { alts: Vec<String>, optional: bool },
    /// Slot imienia persony.
    Persona,
}

/// Skompilowana fraza.
#[derive(Debug, Clone)]
pub(crate) struct Pattern {
    pub kind: CommandKind,
    elems: Vec<Elem>,
}

/// Skompilowana gramatyka.
#[derive(Debug, Clone)]
pub(crate) struct Compiled {
    pub patterns: Vec<Pattern>,
    pub fillers: Vec<String>,
    /// (persona, formy po `fold`, formy adresowania po `fold`).
    pub personas: Vec<(PersonaId, Vec<String>, Vec<String>)>,
}

/// Pozycja w trakcie dopasowania.
#[derive(Debug, Clone)]
struct Cursor {
    i: usize,
    score: f32,
    persona: Option<PersonaId>,
    started: bool,
}

/// Dopasowanie frazy.
#[derive(Debug, Clone)]
pub(crate) struct Match {
    pub end: usize,
    pub score: f32,
    pub persona: Option<PersonaId>,
}

fn compile_phrase(kind: CommandKind, phrase: &str) -> Pattern {
    let elems = phrase
        .split_whitespace()
        .map(|raw| {
            if raw == "{persona}" {
                return Elem::Persona;
            }
            let optional = raw.starts_with('[') && raw.ends_with(']');
            let inner = raw.trim_start_matches('[').trim_end_matches(']');
            Elem::Word {
                alts: inner
                    .split('|')
                    .map(fold)
                    .filter(|a| !a.is_empty())
                    .collect(),
                optional,
            }
        })
        .collect();
    Pattern { kind, elems }
}

impl Compiled {
    /// Kompiluje gramatykę.
    pub(crate) fn new(grammar: &Grammar) -> Self {
        let patterns = grammar
            .rules
            .iter()
            .flat_map(|r| r.phrases.iter().map(move |p| compile_phrase(r.command, p)))
            .filter(|p| !p.elems.is_empty())
            .collect();
        let fold_all = |v: &[String]| v.iter().map(|s| fold(s)).collect::<Vec<_>>();
        Self {
            patterns,
            fillers: fold_all(&grammar.fillers),
            personas: grammar
                .personas
                .iter()
                .map(|p| {
                    (
                        p.persona.clone(),
                        fold_all(&p.forms),
                        fold_all(&p.address_forms),
                    )
                })
                .collect(),
        }
    }

    /// Czy słowo jest wypełniaczem.
    pub(crate) fn is_filler(&self, word: &str) -> bool {
        self.fillers.iter().any(|f| f == word)
    }

    /// Persona, do której słowo się zwraca (mianownik/wołacz).
    pub(crate) fn address(&self, word: &str) -> Option<&PersonaId> {
        self.personas
            .iter()
            .find(|(_, _, address)| address.iter().any(|a| word_score(word, a).is_some()))
            .map(|(id, _, _)| id)
    }

    fn persona_form(&self, word: &str) -> Option<(PersonaId, f32)> {
        self.personas.iter().find_map(|(id, forms, _)| {
            forms
                .iter()
                .filter_map(|f| word_score(word, f))
                .reduce(f32::max)
                .map(|s| (id.clone(), s))
        })
    }

    fn step(&self, elems: &[Elem], words: &[String], at: Cursor, best: &mut Option<Match>) {
        let Some((first, rest)) = elems.split_first() else {
            let better = best
                .as_ref()
                .is_none_or(|b| at.i > b.end || (at.i == b.end && at.score > b.score));
            if better {
                *best = Some(Match {
                    end: at.i,
                    score: at.score,
                    persona: at.persona,
                });
            }
            return;
        };
        if let Some(word) = words.get(at.i) {
            let hit = match first {
                Elem::Word { alts, .. } => alts
                    .iter()
                    .filter_map(|a| word_score(word, a))
                    .reduce(f32::max)
                    .map(|s| (s, at.persona.clone())),
                Elem::Persona => self.persona_form(word).map(|(id, s)| (s, Some(id))),
            };
            if let Some((s, persona)) = hit {
                let next = Cursor {
                    i: at.i + 1,
                    score: at.score * s,
                    persona,
                    started: true,
                };
                self.step(rest, words, next, best);
            }
            // Wypełniacz wewnątrz frazy („przełącz proszę na Deltę”) — pomijany bez kary.
            if at.started && self.is_filler(word) {
                let next = Cursor {
                    i: at.i + 1,
                    ..at.clone()
                };
                self.step(elems, words, next, best);
            }
        }
        if matches!(first, Elem::Word { optional: true, .. }) {
            self.step(rest, words, at, best);
        }
    }

    /// Najdłuższe (potem najpewniejsze) dopasowanie dowolnej frazy od pozycji `i`.
    pub(crate) fn best_at(&self, words: &[String], i: usize) -> Option<(CommandKind, Match)> {
        let mut winner: Option<(CommandKind, Match)> = None;
        for p in &self.patterns {
            let mut best = None;
            let start = Cursor {
                i,
                score: 1.0,
                persona: None,
                started: false,
            };
            self.step(&p.elems, words, start, &mut best);
            let Some(m) = best.filter(|m| m.end > i) else {
                continue;
            };
            if p.kind == CommandKind::SwitchPersona && m.persona.is_none() {
                continue;
            }
            let better = winner
                .as_ref()
                .is_none_or(|(_, w)| m.end > w.end || (m.end == w.end && m.score > w.score));
            if better {
                winner = Some((p.kind, m));
            }
        }
        winner
    }
}
