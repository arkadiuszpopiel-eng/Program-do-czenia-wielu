//! Tokeny normalizatora: słowa, liczby, spacje, symbole i fragmenty już „mówione”.

/// Token tekstu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Tok {
    /// Ciąg liter (Unicode).
    Word(String),
    /// Ciąg cyfr ASCII.
    Num(String),
    /// Ciąg białych znaków.
    Space(String),
    /// Pojedynczy inny znak.
    Sym(char),
    /// Tekst gotowy do mowy (słownik, URL, kod) — reguły go nie dotykają.
    Fixed(String),
}

impl Tok {
    /// Słowo, jeśli token jest słowem.
    pub(crate) fn word(&self) -> Option<&str> {
        match self {
            Tok::Word(w) => Some(w),
            _ => None,
        }
    }

    /// Cyfry, jeśli token jest liczbą.
    pub(crate) fn num(&self) -> Option<&str> {
        match self {
            Tok::Num(n) => Some(n),
            _ => None,
        }
    }

    /// Czy to dany symbol.
    pub(crate) fn is_sym(&self, c: char) -> bool {
        matches!(self, Tok::Sym(s) if *s == c)
    }

    /// Czy to spacja bez końca linii (np. separator tysięcy lub odstęp w zdaniu).
    pub(crate) fn is_inline_space(&self) -> bool {
        matches!(self, Tok::Space(s) if !s.contains('\n'))
    }

    /// Czy to pojedyncza spacja (także twarda) — separator grup tysięcy.
    pub(crate) fn is_single_space(&self) -> bool {
        matches!(self, Tok::Space(s) if matches!(s.as_str(), " " | "\u{a0}" | "\u{202f}"))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Letter,
    Digit,
    Space,
    Other,
}

fn class_of(c: char) -> Class {
    if c.is_ascii_digit() {
        Class::Digit
    } else if c.is_alphabetic() {
        Class::Letter
    } else if c.is_whitespace() {
        Class::Space
    } else {
        Class::Other
    }
}

/// Dzieli tekst na tokeny (sklejenie tokenów odtwarza tekst).
pub(crate) fn tokenize(text: &str, out: &mut Vec<Tok>) {
    let mut cur = String::new();
    let mut cur_class: Option<Class> = None;
    let flush = |cur: &mut String, class: Option<Class>, out: &mut Vec<Tok>| {
        if cur.is_empty() {
            return;
        }
        let s = std::mem::take(cur);
        out.push(match class {
            Some(Class::Letter) => Tok::Word(s),
            Some(Class::Digit) => Tok::Num(s),
            _ => Tok::Space(s),
        });
    };
    for c in text.chars() {
        let class = class_of(c);
        if class == Class::Other {
            flush(&mut cur, cur_class, out);
            cur_class = None;
            out.push(Tok::Sym(c));
            continue;
        }
        if cur_class != Some(class) {
            flush(&mut cur, cur_class, out);
            cur_class = Some(class);
        }
        cur.push(c);
    }
    flush(&mut cur, cur_class, out);
}

/// Indeks następnego tokenu po opcjonalnej spacji w linii.
pub(crate) fn skip_space(toks: &[Tok], i: usize) -> usize {
    match toks.get(i) {
        Some(t) if t.is_inline_space() => i + 1,
        _ => i,
    }
}

/// Czy kropka pod `k` należy do skrótu (dalej małą literą, liczba lub waluta), a nie kończy zdania.
pub(crate) fn dot_is_abbreviation(toks: &[Tok], k: usize) -> bool {
    if !matches!(toks.get(k), Some(t) if t.is_sym('.')) {
        return false;
    }
    if !matches!(toks.get(k + 1), Some(t) if t.is_inline_space()) {
        return matches!(toks.get(k + 1), Some(Tok::Sym(',' | ')' | ';' | ':')));
    }
    match toks.get(k + 2) {
        Some(Tok::Word(w)) => {
            w.chars().next().is_some_and(char::is_lowercase) || super::tables::currency(w).is_some()
        }
        Some(Tok::Num(_) | Tok::Fixed(_)) => true,
        Some(Tok::Sym(c)) => matches!(c, '$' | '€' | '£' | '(' | '-'),
        _ => false,
    }
}

/// Ostatnie słowo wyniku, jeśli wynik kończy się literą (po zdjęciu spacji) — kontekst przypadku.
pub(crate) fn last_word(out: &str) -> Option<String> {
    let trimmed = out.trim_end_matches([' ', '\t', '\u{a0}']);
    if !trimmed.chars().last().is_some_and(char::is_alphabetic) {
        return None;
    }
    let word: String = trimmed
        .chars()
        .rev()
        .take_while(|c| c.is_alphabetic())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Some(word.to_lowercase())
}

/// Dopisuje tekst; wstawia spację między sklejonymi słowem a liczbą („5km” → „pięć kilometrów”).
pub(crate) fn emit(out: &mut String, text: &str) {
    let ends_alnum = out.chars().last().is_some_and(char::is_alphanumeric);
    let starts_alnum = text.chars().next().is_some_and(char::is_alphanumeric);
    if ends_alnum && starts_alnum {
        out.push(' ');
    }
    out.push_str(text);
}

/// Odtwarza token jako tekst.
pub(crate) fn push_tok(out: &mut String, tok: &Tok) {
    match tok {
        Tok::Word(s) | Tok::Num(s) | Tok::Fixed(s) => emit(out, s),
        Tok::Space(s) => out.push_str(s),
        Tok::Sym(c) => out.push(*c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_round_trip() {
        let text = "Mam 5km, 3,50 zł i °C — zażółć!";
        let mut toks = Vec::new();
        tokenize(text, &mut toks);
        let mut out = String::new();
        for t in &toks {
            match t {
                Tok::Word(s) | Tok::Num(s) | Tok::Space(s) | Tok::Fixed(s) => out.push_str(s),
                Tok::Sym(c) => out.push(*c),
            }
        }
        assert_eq!(out, text);
        assert_eq!(toks[2], Tok::Num("5".into()));
        assert_eq!(toks[3], Tok::Word("km".into()));
    }

    #[test]
    fn context_helpers() {
        assert_eq!(last_word("jestem o "), Some("o".into()));
        assert_eq!(last_word("koniec. "), None);
        let mut s = String::from("x");
        emit(&mut s, "pięć");
        assert_eq!(s, "x pięć");
    }
}
