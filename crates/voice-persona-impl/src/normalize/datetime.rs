//! Daty, godziny i lata: liczebniki porządkowe w odpowiednim przypadku.

use super::tables::{self, MONTHS_GEN};
use super::tokens::{Tok, dot_is_abbreviation, skip_space};
use crate::numbers::{OrdForm, cardinal_nom, ordinal};

fn num_in(toks: &[Tok], i: usize, max_len: usize) -> Option<u64> {
    let n = toks.get(i)?.num()?;
    if n.len() > max_len {
        return None;
    }
    n.parse().ok()
}

fn four_digits(toks: &[Tok], i: usize) -> Option<u64> {
    let n = toks.get(i)?.num()?;
    (n.len() == 4).then(|| n.parse().ok()).flatten()
}

enum YearWord {
    /// „rok” (mianownik).
    Rok,
    /// „roku” / „r.” (dopełniacz lub miejscownik).
    Roku,
}

/// Słowo „r.”/„roku”/„rok” po roku; zwraca indeks za nim (kropka „r.” zostaje, gdy kończy zdanie).
fn year_word(toks: &[Tok], j: usize) -> Option<(YearWord, usize)> {
    let k = skip_space(toks, j);
    match toks.get(k)?.word()? {
        "r" if toks.get(k + 1).is_some_and(|t| t.is_sym('.')) => {
            let next = if dot_is_abbreviation(toks, k + 1) {
                k + 2
            } else {
                k + 1
            };
            Some((YearWord::Roku, next))
        }
        "roku" => Some((YearWord::Roku, k + 1)),
        "rok" => Some((YearWord::Rok, k + 1)),
        _ => None,
    }
}

fn say_year(year: u64, form: OrdForm) -> String {
    ordinal(year, form).unwrap_or_else(|| cardinal_nom(year))
}

fn day_gen(day: u64) -> Option<String> {
    (1..=31)
        .contains(&day)
        .then(|| ordinal(day, OrdForm::GenM))
        .flatten()
}

fn month_gen(month: u64) -> Option<&'static str> {
    let idx = usize::try_from(month).ok()?.checked_sub(1)?;
    MONTHS_GEN.get(idx).copied()
}

/// Data z rokiem: „dzień miesiąc rok roku”; pochłania następujące „r.”/„roku”.
fn full_date(
    day: u64,
    month: u64,
    year: u64,
    toks: &[Tok],
    next: usize,
) -> Option<(usize, String)> {
    let text = format!(
        "{} {} {} roku",
        day_gen(day)?,
        month_gen(month)?,
        say_year(year, OrdForm::GenM)
    );
    match year_word(toks, next) {
        Some((YearWord::Roku, after)) => Some((after, text)),
        _ => Some((next, text)),
    }
}

/// `1.10.2026` i `2026-10-01`.
fn numeric_date(toks: &[Tok], i: usize) -> Option<(usize, String)> {
    let sym = |k: usize, c: char| toks.get(k).is_some_and(|t| t.is_sym(c));
    if let Some(year) = four_digits(toks, i) {
        if sym(i + 1, '-') && sym(i + 3, '-') {
            let month = num_in(toks, i + 2, 2)?;
            let day = num_in(toks, i + 4, 2)?;
            return full_date(day, month, year, toks, i + 5);
        }
        return None;
    }
    let day = num_in(toks, i, 2)?;
    if !(sym(i + 1, '.') && sym(i + 3, '.')) {
        return None;
    }
    let month = num_in(toks, i + 2, 2)?;
    let year = four_digits(toks, i + 4)?;
    full_date(day, month, year, toks, i + 5)
}

/// `1 października [2026 [r.]]` oraz zakres `1-5 maja`.
fn day_month(toks: &[Tok], i: usize) -> Option<(usize, String)> {
    let day = num_in(toks, i, 2)?;
    let (day2, j) = match (toks.get(i + 1), num_in(toks, i + 2, 2)) {
        (Some(t), Some(d2)) if t.is_sym('-') || t.is_sym('–') => (Some(d2), i + 3),
        _ => (None, i + 1),
    };
    if !toks.get(j).is_some_and(Tok::is_inline_space) {
        return None;
    }
    let month_word = toks.get(j + 1)?.word()?;
    tables::month_index(month_word)?;
    let mut text = day_gen(day)?;
    if let Some(d2) = day2 {
        text = format!("{text} do {}", day_gen(d2)?);
    }
    text = format!("{text} {month_word}");
    let mut next = j + 2;
    if toks.get(next).is_some_and(Tok::is_inline_space)
        && let Some(year) = four_digits(toks, next + 1)
    {
        text = format!("{text} {} roku", say_year(year, OrdForm::GenM));
        next += 2;
        if let Some((YearWord::Roku, after)) = year_word(toks, next) {
            next = after;
        }
    }
    Some((next, text))
}

/// Rok z „r.”/„roku”/„rok” albo po „w” (1900–2099).
fn year(toks: &[Tok], i: usize, prev: Option<&str>) -> Option<(usize, String)> {
    let y = num_in(toks, i, 4)?;
    let decimal = toks
        .get(i + 1)
        .is_some_and(|t| t.is_sym(',') || t.is_sym('.'))
        && toks.get(i + 2).and_then(Tok::num).is_some();
    if !(1..=2999).contains(&y) || decimal {
        return None;
    }
    let loc = matches!(prev, Some("w" | "we"));
    match year_word(toks, i + 1) {
        Some((YearWord::Rok, next)) => Some((next, format!("{} rok", say_year(y, OrdForm::NomM)))),
        Some((YearWord::Roku, next)) => {
            let form = if loc { OrdForm::LocM } else { OrdForm::GenM };
            Some((next, format!("{} roku", say_year(y, form))))
        }
        None if loc && (1900..=2099).contains(&y) && four_digits(toks, i).is_some() => {
            let k = skip_space(toks, i + 1);
            let unit_follows = toks.get(k).and_then(Tok::word).is_some_and(|w| {
                tables::unit(w).is_some()
                    || tables::currency(w).is_some()
                    || tables::multiplier(w).is_some()
            });
            (!unit_follows).then(|| (i + 1, say_year(y, OrdForm::LocM)))
        }
        None => None,
    }
}

fn hour_form(prev: Option<&str>) -> OrdForm {
    match prev {
        Some("o" | "po" | "od" | "do" | "około" | "koło" | "godzinie" | "godziny") => {
            OrdForm::GenLocF
        }
        Some("przed" | "między" | "pomiędzy" | "na" | "godziną" | "godzinę") => {
            OrdForm::InsAccF
        }
        _ => OrdForm::NomF,
    }
}

fn say_hour(hour: u64, minute: Option<u64>, form: OrdForm) -> Option<String> {
    let h = if hour == 0 {
        "zero".to_owned()
    } else {
        ordinal(hour, form)?
    };
    Some(match minute {
        None | Some(0) => h,
        Some(m) if m < 10 => format!("{h} zero {}", cardinal_nom(m)),
        Some(m) => format!("{h} {}", cardinal_nom(m)),
    })
}

/// Godzina `h:mm` (bez sekund) na pozycji `i`.
fn clock_at(toks: &[Tok], i: usize) -> Option<(u64, u64, usize)> {
    let hour = num_in(toks, i, 2)?;
    if !toks.get(i + 1).is_some_and(|t| t.is_sym(':')) {
        return None;
    }
    let mm = toks.get(i + 2)?.num()?;
    let minute: u64 = if mm.len() == 2 {
        mm.parse().ok()?
    } else {
        return None;
    };
    let seconds = toks.get(i + 3).is_some_and(|t| t.is_sym(':'))
        && toks.get(i + 4).and_then(Tok::num).is_some();
    (hour <= 24 && minute < 60 && !seconds).then_some((hour, minute, i + 3))
}

/// `14:05`, `o 14:05`, `14:00-16:00` oraz `godz. 14`.
fn time(toks: &[Tok], i: usize, prev: Option<&str>) -> Option<(usize, String)> {
    let form = hour_form(prev);
    let Some((hour, minute, next)) = clock_at(toks, i) else {
        let after_godz = matches!(
            prev,
            Some("godzina" | "godziny" | "godzinie" | "godziną" | "godzinę")
        );
        let hour = num_in(toks, i, 2)?;
        return (after_godz && hour <= 24)
            .then(|| say_hour(hour, None, form).map(|h| (i + 1, h)))
            .flatten();
    };
    let mut text = say_hour(hour, Some(minute), form)?;
    let mut end = next;
    let dash = |t: &Tok| t.is_sym('-') || t.is_sym('–');
    let second = if toks.get(next).is_some_and(dash) {
        Some(next + 1)
    } else if toks.get(next).is_some_and(Tok::is_inline_space)
        && toks.get(next + 1).is_some_and(dash)
        && toks.get(next + 2).is_some_and(Tok::is_inline_space)
    {
        Some(next + 3)
    } else {
        None
    };
    if let Some((h2, m2, after)) = second.and_then(|s| clock_at(toks, s)) {
        text = format!("{text} do {}", say_hour(h2, Some(m2), OrdForm::GenLocF)?);
        end = after;
    }
    Some((end, text))
}

/// Reguły dat i czasu dla tokenu liczby.
pub(crate) fn rule(toks: &[Tok], i: usize, prev: Option<&str>) -> Option<(usize, String)> {
    numeric_date(toks, i)
        .or_else(|| day_month(toks, i))
        .or_else(|| time(toks, i, prev))
        .or_else(|| year(toks, i, prev))
}
