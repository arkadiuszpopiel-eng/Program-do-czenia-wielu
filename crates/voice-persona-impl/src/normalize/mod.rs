//! Normalizator tekstu PL do mowy (PLAN §6.7, docs/VOICE.md §12).
//!
//! Potok: kod → URL/e-mail/domeny → słownik wymowy (pierwszeństwo) → tokenizacja → reguły
//! (daty, godziny, lata, waluty, procenty, jednostki, zakresy, telefony, skróty, liczby).
//! Każdy token liczby jest zamieniany na słowa, więc w wyniku nie zostaje żadna cyfra ASCII.

mod abbrev;
mod datetime;
mod nouns;
mod numrules;
mod protect;
mod tables;
mod tokens;

use voice_persona_contract::{Lexicon, TextNormalizer};

use protect::Piece;
use tokens::{Tok, emit, last_word, push_tok, tokenize};

/// Normalizator pl-PL (deterministyczny, bez stanu).
#[derive(Debug, Clone, Copy, Default)]
pub struct PlNormalizer;

impl TextNormalizer for PlNormalizer {
    fn normalize(&self, text: &str, lexicon: &Lexicon) -> String {
        normalize(text, lexicon)
    }
}

fn apply_rules(toks: &[Tok], i: usize, out: &str) -> Option<(usize, String)> {
    let prev = last_word(out);
    let prev = prev.as_deref();
    match toks.get(i)? {
        Tok::Num(_) => {
            datetime::rule(toks, i, prev).or_else(|| numrules::number_rule(toks, i, prev))
        }
        Tok::Word(_) => abbrev::rule(toks, i, prev),
        Tok::Sym(_) => numrules::sym_rule(toks, i, prev),
        Tok::Space(_) | Tok::Fixed(_) => None,
    }
}

/// Normalizuje tekst z użyciem słownika wymowy.
pub fn normalize(text: &str, lexicon: &Lexicon) -> String {
    let mut toks = Vec::new();
    for piece in protect::protect(text, lexicon) {
        match piece {
            Piece::Raw(s) => tokenize(&s, &mut toks),
            Piece::Fixed(s) => toks.push(Tok::Fixed(s)),
        }
    }
    let mut out = String::with_capacity(text.len() + text.len() / 2);
    let mut i = 0;
    while i < toks.len() {
        match apply_rules(&toks, i, &out) {
            Some((next, said)) if next > i => {
                emit(&mut out, &said);
                i = next;
            }
            _ => {
                if let Some(tok) = toks.get(i) {
                    push_tok(&mut out, tok);
                }
                i += 1;
            }
        }
    }
    out
}
