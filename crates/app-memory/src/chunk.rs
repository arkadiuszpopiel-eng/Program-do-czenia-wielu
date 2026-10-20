//! Podział treści „Naucz Alfę” na fragmenty-fakty (P5): akapity → zdania → słowa, pakowane
//! zachłannie do limitu znaków. Każdy fragment to **jedna linia** (białe znaki zwinięte, bez
//! znaków sterujących i przełączników kierunku tekstu) — treść z zewnątrz nie może w kontekście
//! pamięci czatu udawać nowej pozycji listy ani ukrywać fragmentów. Limity: znaki wejścia, znaki
//! fragmentu, liczba faktów; powtórzone fragmenty pomijane.

use std::collections::HashSet;

/// Limity podziału.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Najwięcej znaków jednego fragmentu (fakt mieści się w kontekście czatu obok innych).
    pub chunk_chars: usize,
    /// Najwięcej znaków treści wejściowej (reszta ucięta).
    pub max_input_chars: usize,
    /// Najwięcej faktów z jednej nauki (reszta ucięta).
    pub max_facts: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            chunk_chars: 800,
            max_input_chars: 100_000,
            max_facts: 128,
        }
    }
}

/// Fragment: treść i tura źródłowa (dla nauki z rozmowy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// Pierwsza tura, z której pochodzi fragment (`None` — tekst, plik).
    pub turn: Option<u64>,
    /// Treść (jedna linia).
    pub text: String,
}

/// Wynik podziału.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chunks {
    /// Fragmenty w kolejności źródła.
    pub pieces: Vec<Piece>,
    /// Czy treść ucięto (limit znaków wejścia albo liczby faktów).
    pub truncated: bool,
    /// Pominięte powtórzenia.
    pub duplicates: usize,
}

/// Znaki niewidoczne zmieniające kierunek/układ tekstu (mogłyby ukryć treść przed czytelnikiem).
fn invisible(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

/// Jedna linia: białe znaki zwinięte do pojedynczej spacji, bez znaków sterujących i niewidocznych.
pub fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| !c.is_control() && !invisible(*c))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Akapity tekstu (oddzielone pustą linią), każdy jako jedna linia; puste pominięte.
pub fn paragraphs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            push_paragraph(&mut out, &current);
            current.clear();
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    push_paragraph(&mut out, &current);
    out
}

fn push_paragraph(out: &mut Vec<String>, raw: &str) {
    let line = one_line(raw);
    if !line.is_empty() {
        out.push(line);
    }
}

fn count(s: &str) -> usize {
    s.chars().count()
}

/// Zdania jednej linii (koniec: `.`, `!`, `?`, `…`, `;` przed spacją).
fn sentences(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev_end = false;
    for (i, c) in line.char_indices() {
        if c == ' ' && prev_end {
            out.push(line[start..i].trim());
            start = i;
        }
        prev_end = matches!(c, '.' | '!' | '?' | '…' | ';');
    }
    out.push(line[start..].trim());
    out.retain(|s| !s.is_empty());
    out
}

/// Słowa pakowane do `max`; słowo dłuższe niż `max` cięte po znakach.
fn by_words(sentence: &str, max: usize) -> Vec<String> {
    let mut parts = Vec::new();
    for word in sentence.split(' ') {
        if count(word) <= max {
            parts.push(word.to_owned());
        } else {
            let chars: Vec<char> = word.chars().collect();
            parts.extend(chars.chunks(max).map(|c| c.iter().collect::<String>()));
        }
    }
    pack(parts, max)
}

/// Kawałki ≤ `max` znaków jednego akapitu.
fn split_line(line: &str, max: usize) -> Vec<String> {
    if count(line) <= max {
        return vec![line.to_owned()];
    }
    let mut parts = Vec::new();
    for s in sentences(line) {
        if count(s) <= max {
            parts.push(s.to_owned());
        } else {
            parts.extend(by_words(s, max));
        }
    }
    parts
}

/// Zachłanne łączenie kawałków spacją, dopóki mieszczą się w `max`.
fn pack(parts: Vec<String>, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for part in parts {
        if !current.is_empty() && count(&current) + 1 + count(&part) > max {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(&part);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Pierwsze `n` znaków.
pub fn take_chars(s: &str, n: usize) -> &str {
    s.char_indices().nth(n).map_or(s, |(i, _)| &s[..i])
}

/// Dzieli segmenty (akapity tekstu albo tury rozmowy z numerem) na fragmenty w limitach.
/// Fragmenty z różnych tur nie są łączone (każdy fakt wskazuje jedną turę źródłową).
pub fn chunk(segments: &[(Option<u64>, String)], limits: Limits) -> Chunks {
    let max = limits.chunk_chars.max(1);
    let mut out = Chunks::default();
    let mut budget = limits.max_input_chars;
    let mut seen = HashSet::new();
    let mut group: Vec<String> = Vec::new();
    let mut group_turn: Option<Option<u64>> = None;
    let mut flush = |group: &mut Vec<String>, turn: Option<u64>, out: &mut Chunks| {
        for text in pack(std::mem::take(group), max) {
            if !seen.insert(text.to_lowercase()) {
                out.duplicates += 1;
            } else if out.pieces.len() < limits.max_facts {
                out.pieces.push(Piece { turn, text });
            } else {
                out.truncated = true;
            }
        }
    };
    for (turn, raw) in segments {
        let line = one_line(raw);
        if line.is_empty() {
            continue;
        }
        if budget == 0 {
            out.truncated = true;
            break;
        }
        let line = if count(&line) > budget {
            out.truncated = true;
            take_chars(&line, budget).trim_end().to_owned()
        } else {
            line
        };
        budget = budget.saturating_sub(count(&line));
        if group_turn.is_some_and(|t| t != *turn) {
            flush(&mut group, group_turn.flatten(), &mut out);
        }
        group_turn = Some(*turn);
        group.extend(split_line(&line, max));
    }
    flush(&mut group, group_turn.flatten(), &mut out);
    out
}
