//! Reguły liczb: kwoty, waluty, procenty, jednostki, mnożniki, zakresy, wersje, telefony.

use super::tables::{self, CELSIUS, Currency, KMH, Noun, PERCENT};
use super::tokens::{Tok, dot_is_abbreviation, skip_space};
use crate::numbers::{Gender, MAX_CARDINAL, NumCase, cardinal, digits, read_digit_string};

/// Liczba z tekstu: część całkowita (po scaleniu grup tysięcy) i opcjonalny ułamek.
#[derive(Debug, Clone)]
pub(crate) struct NumLit {
    pub int: String,
    pub frac: Option<String>,
    pub frac_sep: char,
    pub next: usize,
}

impl NumLit {
    /// Wartość części całkowitej, jeśli czytana jako liczebnik.
    pub(crate) fn value(&self) -> Option<u64> {
        if self.int.len() > 1 && self.int.starts_with('0') {
            return None;
        }
        self.int.parse::<u64>().ok().filter(|n| *n <= MAX_CARDINAL)
    }
}

/// Parsuje liczbę od tokenu `i` (musi być `Num`).
pub(crate) fn parse_number(toks: &[Tok], i: usize) -> Option<NumLit> {
    let first = toks.get(i)?.num()?;
    let mut int = first.to_owned();
    let mut j = i + 1;
    if first.len() <= 3 {
        while toks.get(j).is_some_and(Tok::is_single_space)
            && toks
                .get(j + 1)
                .and_then(Tok::num)
                .is_some_and(|n| n.len() == 3)
            && !toks.get(j + 2).is_some_and(|t| t.is_sym(':'))
            && !(toks
                .get(j + 2)
                .is_some_and(|t| t.is_sym('.') || t.is_sym(','))
                && toks
                    .get(j + 3)
                    .and_then(Tok::num)
                    .is_some_and(|n| n.len() != 3))
        {
            int.push_str(toks.get(j + 1).and_then(Tok::num).unwrap_or_default());
            j += 2;
        }
    }
    let mut lit = NumLit {
        int,
        frac: None,
        frac_sep: ',',
        next: j,
    };
    if let (Some(Tok::Sym(c @ (',' | '.'))), Some(Tok::Num(f))) = (toks.get(j), toks.get(j + 1)) {
        let version = toks.get(j + 2).is_some_and(|t| t.is_sym('.'))
            && toks.get(j + 3).and_then(Tok::num).is_some();
        if !version {
            lit.frac = Some(f.clone());
            lit.frac_sep = *c;
            lit.next = j + 2;
        }
    }
    Some(lit)
}

/// Czyta część ułamkową: zera wiodące cyfra po cyfrze, reszta liczebnikiem (≤ 3 cyfry).
fn read_frac(f: &str) -> String {
    if f.len() > 3 {
        return digits(f);
    }
    let zeros = f.len() - f.trim_start_matches('0').len();
    let mut words: Vec<String> = std::iter::repeat_n("zero".to_owned(), zeros).collect();
    let rest = &f[zeros..];
    if !rest.is_empty() {
        words.push(read_digit_string(rest, NumCase::Nom, Gender::Masc));
    }
    words.join(" ")
}

/// Kwota słownie (bez rzeczownika).
pub(crate) fn say_amount(lit: &NumLit, case: NumCase, gender: Gender) -> String {
    match &lit.frac {
        None => read_digit_string(&lit.int, case, gender),
        Some(f) => {
            let sep = if lit.frac_sep == '.' {
                "kropka"
            } else {
                "przecinek"
            };
            let int = read_digit_string(&lit.int, case, Gender::Masc);
            format!("{int} {sep} {}", read_frac(f))
        }
    }
}

fn with_noun(lit: &NumLit, noun: &Noun, case: NumCase) -> String {
    let amount = say_amount(lit, case, noun.gender);
    format!(
        "{amount} {}",
        noun.form(lit.value(), lit.frac.is_some(), case)
    )
}

fn say_money(lit: &NumLit, cur: &Currency, case: NumCase) -> String {
    let frac = match &lit.frac {
        Some(f) if f.len() <= 2 => f,
        Some(_) => return with_noun(lit, &cur.main, case),
        None => return with_noun(lit, &cur.main, case),
    };
    let sub = frac.parse::<u64>().unwrap_or(0) * if frac.len() == 1 { 10 } else { 1 };
    let main_lit = NumLit {
        frac: None,
        ..lit.clone()
    };
    let mut parts = Vec::new();
    if main_lit.value() != Some(0) || sub == 0 {
        parts.push(with_noun(&main_lit, &cur.main, case));
    }
    if sub > 0 {
        let sub_words = cardinal(sub, case, cur.sub.gender);
        parts.push(format!(
            "{sub_words} {}",
            cur.sub.form(Some(sub), false, case)
        ));
    }
    parts.join(" ")
}

enum Suffix {
    Currency(Currency),
    Multiplier(Noun, Option<Currency>),
    Unit(Noun),
    Gender(Gender),
    Plain,
}

fn currency_at(toks: &[Tok], k: usize) -> Option<(Currency, usize)> {
    match toks.get(k)? {
        Tok::Word(w) => tables::currency(w).map(|c| (c, k + 1)),
        Tok::Sym(c) => tables::currency_symbol(*c).map(|cur| (cur, k + 1)),
        _ => None,
    }
}

/// Rozpoznaje to, co stoi po liczbie; zwraca sufiks i indeks za nim.
fn suffix_at(toks: &[Tok], j: usize) -> (Suffix, usize) {
    if toks.get(j).is_some_and(|t| t.is_sym('%')) {
        return (Suffix::Unit(PERCENT), j + 1);
    }
    let k = skip_space(toks, j);
    if toks.get(k).is_some_and(|t| t.is_sym('%')) {
        return (Suffix::Unit(PERCENT), k + 1);
    }
    if toks.get(k).is_some_and(|t| t.is_sym('°')) {
        if toks.get(k + 1).and_then(Tok::word) == Some("C") {
            return (Suffix::Unit(CELSIUS), k + 2);
        }
        let deg = Noun {
            one: "stopień",
            few: "stopnie",
            many: "stopni",
            gen_sg: "stopnia",
            frac: None,
            gender: Gender::Masc,
        };
        return (Suffix::Unit(deg), k + 1);
    }
    if let Some((cur, next)) = currency_at(toks, k) {
        return (Suffix::Currency(cur), next);
    }
    let Some(word) = toks.get(k).and_then(Tok::word) else {
        return (Suffix::Plain, j);
    };
    if let Some(mult) = tables::multiplier(word) {
        let mut next = k + 1;
        if dot_is_abbreviation(toks, next) {
            next += 1;
        }
        let c = skip_space(toks, next);
        if let Some((cur, after)) = currency_at(toks, c) {
            return (Suffix::Multiplier(mult, Some(cur)), after);
        }
        return (Suffix::Multiplier(mult, None), next);
    }
    if word == "km"
        && toks.get(k + 1).is_some_and(|t| t.is_sym('/'))
        && toks.get(k + 2).and_then(Tok::word) == Some("h")
    {
        return (Suffix::Unit(KMH), k + 3);
    }
    if let Some(unit) = tables::unit(word) {
        return (Suffix::Unit(unit), k + 1);
    }
    if k > j {
        return (Suffix::Gender(tables::noun_gender(word)), j);
    }
    (Suffix::Plain, j)
}

fn say_with_suffix(lit: &NumLit, suffix: &Suffix, case: NumCase) -> String {
    match suffix {
        Suffix::Currency(cur) => say_money(lit, cur, case),
        Suffix::Multiplier(mult, cur) => {
            let base = with_noun(lit, mult, case);
            match cur {
                Some(c) => format!("{base} {}", c.main.many),
                None => base,
            }
        }
        Suffix::Unit(noun) => with_noun(lit, noun, case),
        Suffix::Gender(g) => say_amount(lit, case, *g),
        Suffix::Plain => say_amount(lit, case, Gender::Masc),
    }
}

/// Wersja / adres IP: `1.2.3` → „jeden kropka dwa kropka trzy”.
fn version(toks: &[Tok], i: usize) -> Option<(usize, String)> {
    let mut parts = vec![read_digit_string(
        toks.get(i)?.num()?,
        NumCase::Nom,
        Gender::Masc,
    )];
    let mut j = i + 1;
    while toks.get(j).is_some_and(|t| t.is_sym('.')) {
        let Some(n) = toks.get(j + 1).and_then(Tok::num) else {
            break;
        };
        parts.push(read_digit_string(n, NumCase::Nom, Gender::Masc));
        j += 2;
    }
    (parts.len() >= 3).then(|| (j, parts.join(" kropka ")))
}

/// Grupy cyfr telefonu czytane osobno („sześćset, siedemset, osiemset”).
fn phone_groups(toks: &[Tok], i: usize) -> Option<(usize, String)> {
    let mut groups = vec![toks.get(i)?.num()?];
    let mut j = i + 1;
    while toks
        .get(j)
        .is_some_and(|t| t.is_single_space() || t.is_sym('-'))
    {
        let Some(n) = toks.get(j + 1).and_then(Tok::num) else {
            break;
        };
        groups.push(n);
        j += 2;
    }
    if groups.len() < 3 || groups.iter().any(|g| g.len() > 3) {
        return None;
    }
    let said: Vec<String> = groups
        .iter()
        .map(|g| read_digit_string(g, NumCase::Nom, Gender::Masc))
        .collect();
    Some((j, said.join(", ")))
}

/// Reguła dla tokenu liczby (zawsze coś zwraca — żadna cyfra nie zostaje w wyniku).
pub(crate) fn number_rule(toks: &[Tok], i: usize, prev: Option<&str>) -> Option<(usize, String)> {
    if prev.is_some_and(tables::phone_trigger)
        && let Some(hit) = phone_groups(toks, i)
    {
        return Some(hit);
    }
    if let Some(hit) = version(toks, i) {
        return Some(hit);
    }
    let lit = parse_number(toks, i)?;
    let case = tables::num_case(prev);
    if let Some(hit) = range(toks, &lit, case) {
        return Some(hit);
    }
    let (suffix, next) = suffix_at(toks, lit.next);
    Some((next, say_with_suffix(&lit, &suffix, case)))
}

/// Zakres `5-10 minut` → „pięć do dziesięciu minut”.
fn range(toks: &[Tok], lit: &NumLit, case: NumCase) -> Option<(usize, String)> {
    let j = lit.next;
    let dash = |t: &Tok| t.is_sym('-') || t.is_sym('–') || t.is_sym('—');
    let second = if toks.get(j).is_some_and(dash) && toks.get(j + 1).and_then(Tok::num).is_some() {
        j + 1
    } else if toks.get(j).is_some_and(Tok::is_inline_space)
        && toks
            .get(j + 1)
            .is_some_and(|t| t.is_sym('–') || t.is_sym('—'))
        && toks.get(j + 2).is_some_and(Tok::is_inline_space)
        && toks.get(j + 3).and_then(Tok::num).is_some()
    {
        j + 3
    } else {
        return None;
    };
    let lit2 = parse_number(toks, second)?;
    let (suffix, next) = suffix_at(toks, lit2.next);
    let gender = match &suffix {
        Suffix::Unit(n) => n.gender,
        Suffix::Gender(g) => *g,
        _ => Gender::Masc,
    };
    let first = say_amount(lit, case, gender);
    Some((
        next,
        format!(
            "{first} do {}",
            say_with_suffix(&lit2, &suffix, NumCase::Gen)
        ),
    ))
}

/// Reguły zaczynające się od symbolu: waluta z przodu, minus, łącznik słowo-liczba, `+48`.
pub(crate) fn sym_rule(toks: &[Tok], i: usize, prev: Option<&str>) -> Option<(usize, String)> {
    let Tok::Sym(c) = toks.get(i)? else {
        return None;
    };
    let before = i.checked_sub(1).and_then(|p| toks.get(p));
    match c {
        '$' | '€' | '£' => {
            let cur = tables::currency_symbol(*c)?;
            let j = skip_space(toks, i + 1);
            toks.get(j)?.num()?;
            let lit = parse_number(toks, j)?;
            let case = tables::num_case(prev);
            let k = skip_space(toks, lit.next);
            if let Some(mult) = toks.get(k).and_then(Tok::word).and_then(tables::multiplier) {
                let mut next = k + 1;
                if dot_is_abbreviation(toks, next) {
                    next += 1;
                }
                return Some((
                    next,
                    format!("{} {}", with_noun(&lit, &mult, case), cur.main.many),
                ));
            }
            Some((lit.next, say_money(&lit, &cur, case)))
        }
        '-' | '−' => {
            toks.get(i + 1)?.num()?;
            match before {
                Some(Tok::Word(_)) => Some((i + 1, " ".to_owned())),
                None | Some(Tok::Space(_)) | Some(Tok::Sym('(' | '/' | ':')) => {
                    let (next, said) = number_rule(toks, i + 1, prev)?;
                    Some((next, format!("minus {said}")))
                }
                _ => None,
            }
        }
        '+' => {
            let cc = toks.get(i + 1)?.num()?;
            if cc.len() > 3 || !matches!(before, None | Some(Tok::Space(_)) | Some(Tok::Sym('('))) {
                return None;
            }
            let j = skip_space(toks, i + 2);
            let (next, groups) = phone_groups(toks, j)?;
            let code = read_digit_string(cc, NumCase::Nom, Gender::Masc);
            Some((next, format!("plus {code}, {groups}")))
        }
        _ => None,
    }
}
