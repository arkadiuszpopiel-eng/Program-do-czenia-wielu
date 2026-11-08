//! Normalizacja odwrotna dyktowania PL: transkrypt STT → tekst do wpisania.
//!
//! - Komendy interpunkcji (bez względu na wielkość liter, polskie znaki i interpunkcję dodaną
//!   przez STT): „kropka”, „przecinek”, „znak zapytania”/„pytajnik”, „wykrzyknik”/„znak
//!   wykrzyknienia”, „dwukropek”, „średnik”, „myślnik”, „łącznik”, „trzy kropki”/„wielokropek”,
//!   „nowa linia”/„nowy wiersz”/„następna linia”, „nowy akapit”, „otwórz/zamknij nawias”,
//!   „otwórz/zamknij cudzysłów” (polskie „ ”). „dosłownie X” wpisuje słowo X bez interpretacji.
//! - Interpunkcja wstawiona przez STT zostaje (`keep_stt_punctuation`), ale komenda ją zastępuje
//!   („Ania, przecinek” → „Ania,” — bez podwójnego znaku).
//! - Liczby: [`crate::numbers`] (tylko jednoznaczne), ułamek „trzy przecinek pięć” → „3,5”,
//!   „dwadzieścia procent” → „20%”.
//! - Odstępy: bez spacji przed `. , ? ! : ; … ) ”`, bez spacji po `( „` i nowej linii; wielka
//!   litera po końcu zdania i nowej linii (oraz na starcie, gdy `capitalize_start`).

use personas_contract::fold;

use crate::numbers::{is_number_word, parse_prefix, unambiguous};

/// Kontekst między frazami (co wpisano ostatnio).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextContext {
    /// Ostatni wpisany znak (`None` — początek pola/sesji).
    pub last: Option<char>,
    /// Następne słowo wielką literą.
    pub capitalize: bool,
    /// Ostatni znak pochodzi z interpunkcji STT (komenda może go zastąpić).
    pub stt_punct: bool,
    /// Otwarty cudzysłów „.
    pub quote_open: bool,
}

impl TextContext {
    /// Początek sesji.
    pub fn start(capitalize: bool) -> Self {
        Self {
            last: None,
            capitalize,
            stt_punct: false,
            quote_open: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Piece {
    Word(String),
    /// Znak interpunkcyjny bez spacji przed (`.`), z flagą „z STT”.
    Close(&'static str, bool),
    /// Bez spacji po (`(`, `„`).
    Open(&'static str),
    /// Ze spacjami (`–`).
    Spaced(&'static str),
    /// Bez spacji z obu stron (`-`).
    Join(&'static str),
    Newline(usize),
}

const COMMANDS: [(&str, &str); 22] = [
    ("znak zapytania", "?"),
    ("znak wykrzyknienia", "!"),
    ("nowa linia", "\n"),
    ("nowy wiersz", "\n"),
    ("nastepna linia", "\n"),
    ("nowy akapit", "\n\n"),
    ("otworz nawias", "("),
    ("zamknij nawias", ")"),
    ("otworz cudzyslow", "„"),
    ("zamknij cudzyslow", "”"),
    ("trzy kropki", "…"),
    ("kropka", "."),
    ("przecinek", ","),
    ("pytajnik", "?"),
    ("wykrzyknik", "!"),
    ("dwukropek", ":"),
    ("srednik", ";"),
    ("myslnik", "–"),
    ("lacznik", "-"),
    ("wielokropek", "…"),
    ("cudzyslow", "\""),
    ("enter", "\n"),
];

fn core(token: &str) -> String {
    fold(token)
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_owned()
}

fn command_at(words: &[&str], i: usize) -> Option<(&'static str, usize)> {
    COMMANDS.iter().find_map(|(phrase, sym)| {
        let parts: Vec<&str> = phrase.split(' ').collect();
        let fits = parts
            .iter()
            .enumerate()
            .all(|(k, p)| words.get(i + k).is_some_and(|w| core(w) == *p));
        fits.then_some((*sym, parts.len()))
    })
}

fn trailing_punct(token: &str) -> Option<&'static str> {
    match token.chars().last()? {
        '.' => Some("."),
        ',' => Some(","),
        '?' => Some("?"),
        '!' => Some("!"),
        ':' => Some(":"),
        ';' => Some(";"),
        '…' => Some("…"),
        _ => None,
    }
}

fn stripped(token: &str) -> &str {
    token.trim_end_matches(['.', ',', '?', '!', ':', ';', '…'])
}

fn pieces(text: &str) -> Vec<Piece> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<Piece> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if core(words[i]) == "doslownie" && i + 1 < words.len() {
            out.push(Piece::Word(stripped(words[i + 1]).to_owned()));
            i += 2;
            continue;
        }
        if let Some((value, n)) = number_at(&words, i) {
            out.extend(value);
            i += n;
            continue;
        }
        if let Some((sym, n)) = command_at(&words, i) {
            // Komenda zastępuje interpunkcję STT tuż przed nią.
            if matches!(out.last(), Some(Piece::Close(_, true))) {
                out.pop();
            }
            out.push(match sym {
                "\n" => Piece::Newline(1),
                "\n\n" => Piece::Newline(2),
                "(" | "„" => Piece::Open(sym),
                "–" => Piece::Spaced(sym),
                "-" => Piece::Join(sym),
                _ => Piece::Close(sym, false),
            });
            i += n;
            continue;
        }
        let w = words[i];
        let word = stripped(w);
        if !word.is_empty() {
            out.push(Piece::Word(word.to_owned()));
        }
        if let Some(p) = trailing_punct(w) {
            out.push(Piece::Close(p, true));
        }
        i += 1;
    }
    out
}

/// Liczebnik od `i` (z ułamkiem i procentem) → kawałki tekstu; `None`, gdy to nie liczba
/// albo zapis cyframi byłby niejednoznaczny.
fn number_at(words: &[&str], i: usize) -> Option<(Vec<Piece>, usize)> {
    // Interpunkcja STT rozdziela liczby („dwadzieścia, trzy” to nie 23).
    let end = words[i..]
        .iter()
        .position(|w| trailing_punct(w).is_some())
        .map_or(words.len(), |k| i + k + 1);
    let cores: Vec<String> = words[i..end].iter().map(|w| core(w)).collect();
    let refs: Vec<&str> = cores.iter().map(String::as_str).collect();
    let (value, n) = parse_prefix(&refs)?;
    let last_raw = words[i + n - 1];
    let mut used = n;
    let mut text = value.to_string();
    let mut digits = unambiguous(value, n);
    // Ułamek dziesiętny: „trzy przecinek pięć” (przecinek bez interpunkcji STT między).
    if refs.get(n) == Some(&"przecinek")
        && trailing_punct(last_raw).is_none()
        && refs.get(n + 1).is_some_and(|w| is_number_word(w))
        && let Some((frac, m)) = parse_prefix(&refs[n + 1..])
    {
        text = format!("{value},{frac}");
        used = n + 1 + m;
        digits = true;
    }
    let tail = words[i + used - 1];
    if refs.get(used).is_some_and(|w| w.starts_with("procent")) && trailing_punct(tail).is_none() {
        text.push('%');
        used += 1;
        digits = true;
    }
    if !digits {
        return None;
    }
    let mut out = vec![Piece::Word(text)];
    if let Some(p) = trailing_punct(words[i + used - 1]) {
        out.push(Piece::Close(p, true));
    }
    Some((out, used))
}

fn capitalized(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Normalizuje frazę i dokleja ją do kontekstu: zwraca tekst do wpisania (z wiodącą spacją, jeśli
/// trzeba) i aktualizuje kontekst.
pub fn normalize(text: &str, ctx: &mut TextContext) -> String {
    let mut out = String::new();
    let space_before = |out: &String, ctx: &TextContext| {
        let prev = out.chars().last().or(ctx.last);
        prev.is_some_and(|c| !matches!(c, ' ' | '\n' | '(' | '„' | '-'))
    };
    for p in pieces(text) {
        match p {
            Piece::Word(w) => {
                if space_before(&out, ctx) {
                    out.push(' ');
                }
                if ctx.capitalize {
                    out.push_str(&capitalized(&w));
                } else {
                    out.push_str(&w);
                }
                ctx.capitalize = false;
                ctx.stt_punct = false;
            }
            Piece::Close(sym, from_stt) => {
                let sym = if sym == "\"" { quote(ctx) } else { sym };
                if sym == "„" {
                    if space_before(&out, ctx) {
                        out.push(' ');
                    }
                } else {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                }
                out.push_str(sym);
                ctx.capitalize |= matches!(sym, "." | "?" | "!" | "…");
                ctx.stt_punct = from_stt;
            }
            Piece::Open(sym) => {
                if space_before(&out, ctx) {
                    out.push(' ');
                }
                out.push_str(sym);
                ctx.quote_open |= sym == "„";
                ctx.stt_punct = false;
            }
            Piece::Spaced(sym) => {
                out.push_str(if space_before(&out, ctx) { " " } else { "" });
                out.push_str(sym);
                out.push(' ');
                ctx.stt_punct = false;
            }
            Piece::Join(sym) => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push_str(sym);
                ctx.stt_punct = false;
            }
            Piece::Newline(n) => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push_str(&"\n".repeat(n));
                ctx.capitalize = true;
                ctx.stt_punct = false;
            }
        }
        if let Some(c) = out.chars().last() {
            ctx.last = Some(c);
        }
    }
    out
}

fn quote(ctx: &mut TextContext) -> &'static str {
    ctx.quote_open = !ctx.quote_open;
    if ctx.quote_open { "„" } else { "”" }
}
