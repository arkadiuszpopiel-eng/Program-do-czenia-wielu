//! Podział tekstu na zdania do czytania (z offsetami znaków — podświetlanie w UI): koniec zdania
//! na `. ! ? …` i nowej linii, z wyjątkiem skrótów PL („np.”, „itd.”, „dr”, „ul.”, „m.in.”…),
//! liczb („3.5”, „12.10.2026”) i inicjałów („J. Kowalski”); zdania dłuższe niż `max_chars`
//! dzielone na przecinkach / spacjach. Puste fragmenty pomijane.

use serde::{Deserialize, Serialize};

/// Skróty, po których kropka nie kończy zdania (małe litery, bez kropki).
const ABBREVIATIONS: [&str; 34] = [
    "np", "itd", "itp", "tzn", "tj", "tzw", "dr", "prof", "mgr", "inż", "ul", "al", "pl", "os",
    "godz", "min", "ok", "ds", "nr", "str", "tel", "wg", "ww", "zob", "por", "św", "im", "jw",
    "pkt", "art", "ust", "poz", "red", "in",
];

/// Fragment do przeczytania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    /// Indeks pierwszego znaku (w znakach `char`, nie bajtach).
    pub start: usize,
    /// Indeks za ostatnim znakiem.
    pub end: usize,
    /// Tekst (bez białych znaków na brzegach).
    pub text: String,
}

fn word_before(chars: &[char], dot: usize) -> String {
    let mut i = dot;
    while i > 0 && chars[i - 1].is_alphanumeric() {
        i -= 1;
    }
    chars[i..dot].iter().collect::<String>().to_lowercase()
}

fn is_boundary(chars: &[char], i: usize) -> bool {
    let c = chars[i];
    if c == '\n' {
        return true;
    }
    if !matches!(c, '.' | '!' | '?' | '…') {
        return false;
    }
    let next = chars.get(i + 1).copied();
    // Ciąg znaków końca („?!”, „...”) — granica na ostatnim.
    if next.is_some_and(|n| matches!(n, '.' | '!' | '?' | '…' | '"' | '”' | ')')) {
        return false;
    }
    if next.is_some_and(|n| !n.is_whitespace()) {
        return false; // „3.5”, „www.alfa.pl”, „m.in.”
    }
    if c == '.' {
        let w = word_before(chars, i);
        let initial = w.chars().count() == 1 && w.chars().all(char::is_alphabetic);
        let digits = !w.is_empty() && w.chars().all(|d| d.is_ascii_digit());
        let next_word_lower = chars[i + 1..]
            .iter()
            .find(|c| !c.is_whitespace())
            .is_some_and(|c| c.is_lowercase());
        if ABBREVIATIONS.contains(&w.as_str()) || initial || (digits && next_word_lower) {
            return false;
        }
    }
    true
}

fn push(out: &mut Vec<Segment>, chars: &[char], start: usize, end: usize, max: usize) {
    let mut s = start;
    while s < end && chars[s].is_whitespace() {
        s += 1;
    }
    let mut e = end;
    while e > s && chars[e - 1].is_whitespace() {
        e -= 1;
    }
    if s >= e {
        return;
    }
    if e - s <= max {
        out.push(Segment {
            start: s,
            end: e,
            text: chars[s..e].iter().collect(),
        });
        return;
    }
    // Za długie: cięcie na ostatnim przecinku/średniku albo spacji przed limitem.
    let limit = s + max;
    let cut = (s + max / 2..limit)
        .rev()
        .find(|&k| matches!(chars[k], ',' | ';' | ':'))
        .map(|k| k + 1)
        .or_else(|| (s + 1..limit).rev().find(|&k| chars[k].is_whitespace()))
        .unwrap_or(limit);
    push(out, chars, s, cut, max);
    push(out, chars, cut, e, max);
}

/// Dzieli tekst na fragmenty ≤ `max_chars` (≥ 20).
pub fn segment(text: &str, max_chars: usize) -> Vec<Segment> {
    let max = max_chars.max(20);
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut start = 0;
    for i in 0..chars.len() {
        if is_boundary(&chars, i) {
            push(&mut out, &chars, start, i + 1, max);
            start = i + 1;
        }
    }
    push(&mut out, &chars, start, chars.len(), max);
    out
}
