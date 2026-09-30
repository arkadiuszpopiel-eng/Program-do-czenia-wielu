//! Normalizacja tekstu polskiego do wyszukiwania (FTS5 `unicode61` nie składa „ł”, spike i).
//!
//! [`fold_pl`] zamienia każdy znak na jeden znak bazowy (długość w znakach się nie zmienia, więc
//! pozycje trafień w tekście złożonym = pozycje w oryginale — potrzebne do podświetleń).

use unicode_normalization::char::{decompose_canonical, is_combining_mark};

/// Składa jeden znak: `ł→l`, `Ł→L` (brak rozkładu Unicode), znaki z diakrytykami → litera bazowa
/// (rozkład NFD bez znaków łączących, np. `ż→z`, `ó→o`, `é→e`). Wielkość liter zachowana.
pub fn fold_char(c: char) -> char {
    match c {
        'ł' => return 'l',
        'Ł' => return 'L',
        'đ' => return 'd',
        'Đ' => return 'D',
        'ø' => return 'o',
        'Ø' => return 'O',
        'ħ' => return 'h',
        'Ħ' => return 'H',
        'ı' => return 'i',
        _ => {}
    }
    if c.is_ascii() {
        return c;
    }
    let mut base: Option<char> = None;
    let mut only_marks_after_base = true;
    decompose_canonical(c, |d| {
        if base.is_none() {
            base = Some(d);
        } else if !is_combining_mark(d) {
            only_marks_after_base = false;
        }
    });
    match base {
        Some(b) if only_marks_after_base && !is_combining_mark(b) => b,
        _ => c,
    }
}

/// Składa tekst znak po znaku ([`fold_char`]); liczba znaków wyniku = liczba znaków wejścia.
///
/// ```
/// assert_eq!(lib_sqlstore::fold_pl("Żółć łąki"), "Zolc laki");
/// ```
pub fn fold_pl(text: &str) -> String {
    text.chars().map(fold_char).collect()
}

/// Słowo tekstu: forma do wyszukiwania + zakres w znakach (nie bajtach) oryginału.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Forma znormalizowana: złożona ([`fold_pl`]), małe litery, bez znaków łączących.
    pub term: String,
    /// Indeks pierwszego znaku słowa w oryginale.
    pub start: usize,
    /// Indeks znaku za ostatnim znakiem słowa.
    pub end: usize,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || is_combining_mark(c)
}

/// Dzieli tekst na słowa (litery/cyfry; znaki łączące należą do słowa) z pozycjami w znakach.
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut current: Option<Token> = None;
    for (idx, c) in text.chars().enumerate() {
        if is_word_char(c) {
            let token = current.get_or_insert_with(|| Token {
                term: String::new(),
                start: idx,
                end: idx,
            });
            token.end = idx + 1;
            if !is_combining_mark(c) {
                token.term.extend(fold_char(c).to_lowercase());
            }
        } else if let Some(token) = current.take().filter(|t| !t.term.is_empty()) {
            out.push(token);
        }
    }
    if let Some(token) = current.filter(|t| !t.term.is_empty()) {
        out.push(token);
    }
    out
}

/// Formy wyszukiwania słów tekstu (bez pozycji), w kolejności wystąpienia.
pub fn search_tokens(text: &str) -> Vec<String> {
    tokenize(text).into_iter().map(|t| t.term).collect()
}

/// Bezpieczne wyrażenie `MATCH` dla FTS5 z tekstu użytkownika: każde słowo jako fraza z prefiksem
/// (`"zolc"*`), słowa połączone AND. `None`, gdy tekst nie ma słów. Składnia FTS5 użytkownika
/// (operatory, kolumny, nawiasy) nie przechodzi — wszystko jest cytowane.
pub fn fts5_match(text: &str) -> Option<String> {
    let terms = search_tokens(text);
    if terms.is_empty() {
        return None;
    }
    let parts: Vec<String> = terms
        .iter()
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    Some(parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polish_letters_fold_to_ascii() {
        assert_eq!(fold_pl("ąćęłńóśźż"), "acelnoszz");
        assert_eq!(fold_pl("ĄĆĘŁŃÓŚŹŻ"), "ACELNOSZZ");
        assert_eq!(fold_pl("Zażółć gęślą jaźń"), "Zazolc gesla jazn");
    }

    #[test]
    fn zolc_finds_zolc_with_diacritics() {
        assert_eq!(search_tokens("żółć"), search_tokens("zolc"));
        assert_eq!(search_tokens("ŻÓŁĆ"), vec!["zolc".to_owned()]);
        assert_eq!(fts5_match("Żółć!"), Some("\"zolc\"*".into()));
    }

    #[test]
    fn other_latin_diacritics_and_non_latin_kept() {
        assert_eq!(fold_pl("éèüñçšžő"), "eeuncszo");
        assert_eq!(fold_pl("Ωμέγα"), "Ωμεγα");
        assert_eq!(fold_pl("日本"), "日本");
    }

    #[test]
    fn nfd_input_combining_marks_join_word() {
        let nfd = "z\u{307}o\u{301}l\u{327}c";
        let tokens = tokenize(nfd);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].term, "zolc");
        assert_eq!((tokens[0].start, tokens[0].end), (0, 7));
    }

    #[test]
    fn tokenize_reports_char_spans() {
        let tokens = tokenize("Ala ma kota, żółć!");
        let spans: Vec<(usize, usize)> = tokens.iter().map(|t| (t.start, t.end)).collect();
        assert_eq!(spans, vec![(0, 3), (4, 6), (7, 11), (13, 17)]);
        assert_eq!(tokens[3].term, "zolc");
    }

    #[test]
    fn fts5_match_neutralizes_syntax() {
        assert_eq!(fts5_match("  ,;  "), None);
        assert_eq!(
            fts5_match("a OR b NEAR(c) text:\"x\""),
            Some("\"a\"* \"or\"* \"b\"* \"near\"* \"c\"* \"text\"* \"x\"*".into())
        );
    }
}
