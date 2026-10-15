//! Rachunki na datach z zegara tego komputera: „ile dni do 24 grudnia”, „ile dni minęło od
//! 1 września”, „jaki dzień tygodnia był 1 stycznia 2000”, „jaka data będzie za 30 dni”,
//! „jaki dzień będzie jutro”. Data bez roku: najbliższa w przód (do) albo w tył (od).

use std::sync::OnceLock;

use chrono::{Datelike, Days, Months, NaiveDate, NaiveDateTime};
use regex::Regex;

use crate::clock::{DAYS, MONTHS};
use crate::text::plural;

/// Dzień i miesiąc słowem albo liczbą, opcjonalnie rok: „24 grudnia”, „24.12.2026”, „2026-12-24”.
const DATE: &str = r"(?:(\d{1,2}) (stycznia|lutego|marca|kwietnia|maja|czerwca|lipca|sierpnia|wrzesnia|pazdziernika|listopada|grudnia)(?: (\d{4}))?(?: r| roku)?|(\d{1,2})\.(\d{1,2})(?:\.(\d{4}))?|(\d{4})-(\d{2})-(\d{2}))";
const FOLDED_MONTHS: [&str; 12] = [
    "stycznia",
    "lutego",
    "marca",
    "kwietnia",
    "maja",
    "czerwca",
    "lipca",
    "sierpnia",
    "wrzesnia",
    "pazdziernika",
    "listopada",
    "grudnia",
];
/// Najdłuższe przesunięcie (dni) — dalej to już nie rachunek „na kalendarzu”.
const MAX_DAYS: u64 = 100_000;

/// Kierunek liczenia daty bez roku.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lean {
    Future,
    Past,
    Near,
}

fn patterns() -> Option<&'static [Regex; 4]> {
    static CELL: OnceLock<Option<[Regex; 4]>> = OnceLock::new();
    CELL.get_or_init(|| {
        let until = format!(
            r"^(?:ile (?:jeszcze |zostalo |zostaje )?(?:dni )?(?:zostalo |zostaje )?do|za ile dni (?:jest |bedzie |wypada )?) ?{DATE}$"
        );
        let since =
            format!(r"^ile (?:dni )?(?:minelo |uplynelo |juz )?(?:dni )?od {DATE}(?: minelo)?$");
        let weekday = format!(
            r"^(?:jaki|jakim|ktory) (?:to |jest |byl |bedzie )?dzien(?:iem)? tygodnia (?:to |jest |byl |bedzie |wypada |przypada )?(?:w |na )?{DATE}$|^(?:jakim dniem tygodnia|jaki dzien tygodnia) (?:jest|byl|bedzie|wypada|przypada) {DATE}$"
        );
        let shift = r"^(?:(?:jaka|ktora) data|(?:jaki|ktory) (?:to )?dzien) (?:bedzie |byla |byl |jest |to )?(?:(za) (\d{1,6} )?(dni|dzien|tydzien|tygodnie|tygodni|miesiac|miesiace|miesiecy|rok|lata|lat)|(\d{1,6}) (dni|dzien|tydzien|tygodnie|tygodni|miesiac|miesiace|miesiecy|rok|lata|lat) temu|(jutro|pojutrze|wczoraj|przedwczoraj))(?: bedzie| byl| byla)?$";
        Some([
            Regex::new(&until).ok()?,
            Regex::new(&since).ok()?,
            Regex::new(&weekday).ok()?,
            Regex::new(shift).ok()?,
        ])
    })
    .as_ref()
}

/// Data z grup wzorca `DATE` (od indeksu `at`); rok domyślny wg kierunku względem `today`.
fn parse(c: &regex::Captures<'_>, at: usize, today: NaiveDate, lean: Lean) -> Option<NaiveDate> {
    let num = |i: usize| c.get(at + i).and_then(|m| m.as_str().parse::<u32>().ok());
    let (day, month, year) = if let Some(name) = c.get(at + 1) {
        let month = FOLDED_MONTHS.iter().position(|m| *m == name.as_str())? as u32 + 1;
        (num(0)?, month, num(2))
    } else if let Some(day) = num(3) {
        (day, num(4)?, num(5))
    } else {
        (num(8)?, num(7)?, num(6))
    };
    if let Some(y) = year {
        return NaiveDate::from_ymd_opt(i32::try_from(y).ok()?, month, day);
    }
    let this = NaiveDate::from_ymd_opt(today.year(), month, day);
    let next = || NaiveDate::from_ymd_opt(today.year() + 1, month, day);
    let prev = || NaiveDate::from_ymd_opt(today.year() - 1, month, day);
    match (lean, this) {
        (Lean::Future, Some(d)) if d < today => next(),
        (Lean::Past, Some(d)) if d > today => prev(),
        (_, Some(d)) => Some(d),
        (Lean::Future, None) => next(),
        (Lean::Past, None) => prev(),
        (Lean::Near, None) => None,
    }
}

/// „24 grudnia 2026”.
fn short(d: NaiveDate) -> String {
    let month = MONTHS.get(d.month0() as usize).copied().unwrap_or_default();
    format!("{} {month} {}", d.day(), d.year())
}

/// „24 grudnia 2026 (czwartek)”.
fn long(d: NaiveDate) -> String {
    format!("{} ({})", short(d), weekday(d))
}

/// Jednostka z polskimi znakami (wzorzec widzi tekst po `fold`).
fn spelled(unit: &str) -> &str {
    match unit {
        "dzien" => "dzień",
        "tydzien" => "tydzień",
        "miesiac" => "miesiąc",
        "miesiace" => "miesiące",
        "miesiecy" => "miesięcy",
        other => other,
    }
}

fn days(n: u64) -> String {
    format!("{n} {}", plural(n, "dzień", "dni", "dni"))
}

/// Przesunięcie o `n` jednostek (w przód albo w tył).
fn shifted(today: NaiveDate, n: u64, unit: &str, forward: bool) -> Option<NaiveDate> {
    if n > MAX_DAYS {
        return None;
    }
    let by_days = |k: u64| {
        let span = Days::new(k.checked_mul(n)?);
        if forward {
            today.checked_add_days(span)
        } else {
            today.checked_sub_days(span)
        }
    };
    let by_months = |k: u32| {
        let span = Months::new(k.checked_mul(u32::try_from(n).ok()?)?);
        if forward {
            today.checked_add_months(span)
        } else {
            today.checked_sub_months(span)
        }
    };
    match unit {
        "dni" | "dzien" => by_days(1),
        "tydzien" | "tygodnie" | "tygodni" => by_days(7),
        "miesiac" | "miesiace" | "miesiecy" => by_months(1),
        _ => by_months(12),
    }
}

fn until(c: &regex::Captures<'_>, today: NaiveDate) -> Option<String> {
    let date = parse(c, 1, today, Lean::Future)?;
    let n = (date - today).num_days();
    Some(match n {
        0 => format!("To dziś — {}.", long(date)),
        n if n > 0 => format!("Do {} zostało **{}**.", long(date), days(n.unsigned_abs())),
        n => format!("{} było **{}** temu.", long(date), days(n.unsigned_abs())),
    })
}

fn since(c: &regex::Captures<'_>, today: NaiveDate) -> Option<String> {
    let date = parse(c, 1, today, Lean::Past)?;
    let n = (today - date).num_days();
    Some(match n {
        0 => format!("To dziś — {}.", long(date)),
        n if n > 0 => format!("Od {} minęło **{}**.", long(date), days(n.unsigned_abs())),
        n => format!("{} będzie za **{}**.", long(date), days(n.unsigned_abs())),
    })
}

fn shift(c: &regex::Captures<'_>, today: NaiveDate) -> Option<String> {
    let (date, label) = if let Some(word) = c.get(6) {
        let (k, forward) = match word.as_str() {
            "jutro" => (1, true),
            "pojutrze" => (2, true),
            "wczoraj" => (1, false),
            _ => (2, false),
        };
        let date = shifted(today, k, "dni", forward)?;
        let mut chars = word.as_str().chars();
        let first = chars.next()?.to_ascii_uppercase();
        (date, format!("{first}{}", chars.as_str()))
    } else if c.get(1).is_some() {
        let unit = c.get(3)?.as_str();
        match c.get(2) {
            Some(m) => {
                let n: u64 = m.as_str().trim().parse().ok()?;
                let label = format!("Za {n} {}", spelled(unit));
                (shifted(today, n, unit, true)?, label)
            }
            None => (
                shifted(today, 1, unit, true)?,
                format!("Za {}", spelled(unit)),
            ),
        }
    } else {
        let n: u64 = c.get(4)?.as_str().parse().ok()?;
        let unit = c.get(5)?.as_str();
        let label = format!("{n} {} temu", spelled(unit));
        (shifted(today, n, unit, false)?, label)
    };
    Some(format!("{label}: **{}**.", long(date)))
}

/// Odpowiedź albo `None` (`text` po `normalize`).
pub fn answer(text: &str, now: NaiveDateTime) -> Option<String> {
    let [until_re, since_re, weekday_re, shift_re] = patterns()?;
    let today = now.date();
    if let Some(c) = until_re.captures(text) {
        return until(&c, today);
    }
    if let Some(c) = since_re.captures(text) {
        return since(&c, today);
    }
    if let Some(c) = weekday_re.captures(text) {
        // Dwie alternatywy wzorca — data w pierwszej (grupy 1–9) albo w drugiej (10–18).
        let at = if c.get(1).is_some() || c.get(4).is_some() || c.get(7).is_some() {
            1
        } else {
            10
        };
        let date = parse(&c, at, today, Lean::Near)?;
        return Some(format!("{} to **{}**.", short(date), weekday(date)));
    }
    let c = shift_re.captures(text)?;
    shift(&c, today)
}

fn weekday(d: NaiveDate) -> &'static str {
    DAYS.get(d.weekday().num_days_from_monday() as usize)
        .copied()
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "dates_tests.rs"]
mod tests;
