//! Fakty kalendarzowe bez modelu: „czy 2028 jest rokiem przestępnym”, „ile dni ma luty 2028”,
//! „ile dni ma ten miesiąc”, „ile dni ma rok 2027”. Miesiąc bez roku — bieżący rok.

use std::sync::OnceLock;

use chrono::{Datelike, NaiveDate, NaiveDateTime};
use regex::Regex;

use crate::text::re;

const LEAP: &str = r"^czy (?:rok )?(\d{1,4}) (?:to |jest |byl |bedzie |byl to |bedzie to )?(?:rok |rokiem )?przestepn(?:y|ym)$";
const MONTH: &str = r"^ile dni (?:ma|liczy|bedzie mial|mial) (styczen|luty|marzec|kwiecien|maj|czerwiec|lipiec|sierpien|wrzesien|pazdziernik|listopad|grudzien|ten miesiac|biezacy miesiac|obecny miesiac)(?: (?:w )?(?:roku )?(\d{1,4}))?$";
const YEAR: &str =
    r"^ile dni (?:ma|liczy|bedzie mial|mial) (?:rok (\d{1,4})|ten rok|biezacy rok|obecny rok)$";

const FOLDED: [&str; 12] = [
    "styczen",
    "luty",
    "marzec",
    "kwiecien",
    "maj",
    "czerwiec",
    "lipiec",
    "sierpien",
    "wrzesien",
    "pazdziernik",
    "listopad",
    "grudzien",
];
const SHOWN: [&str; 12] = [
    "Styczeń",
    "Luty",
    "Marzec",
    "Kwiecień",
    "Maj",
    "Czerwiec",
    "Lipiec",
    "Sierpień",
    "Wrzesień",
    "Październik",
    "Listopad",
    "Grudzień",
];

fn leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Liczba dni miesiąca (1–12) w roku.
fn month_days(year: i32, month: u32) -> Option<i64> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?
    };
    Some((next - first).num_days())
}

fn year_of(m: Option<regex::Match<'_>>, now: NaiveDateTime) -> Option<i32> {
    match m {
        Some(m) => m.as_str().parse().ok().filter(|y| *y >= 1),
        None => Some(now.year()),
    }
}

/// Odpowiedź albo `None` (`text` po `normalize`).
pub fn answer(text: &str, now: NaiveDateTime) -> Option<String> {
    static LEAP_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static MONTH_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static YEAR_RE: OnceLock<Option<Regex>> = OnceLock::new();
    if let Some(c) = re(&LEAP_RE, LEAP)?.captures(text) {
        let year = year_of(c.get(1), now)?;
        return Some(if leap(year) {
            format!("{year} **jest** rokiem przestępnym (366 dni, luty ma 29 dni).")
        } else {
            format!("{year} **nie jest** rokiem przestępnym (365 dni, luty ma 28 dni).")
        });
    }
    if let Some(c) = re(&MONTH_RE, MONTH)?.captures(text) {
        let name = c.get(1)?.as_str();
        let year = year_of(c.get(2), now)?;
        let month = match FOLDED.iter().position(|m| *m == name) {
            Some(i) => u32::try_from(i).ok()? + 1,
            None => now.month(),
        };
        let shown = SHOWN.get(usize::try_from(month).ok()? - 1)?;
        let days = month_days(year, month)?;
        return Some(format!("{shown} {year} ma **{days} dni**."));
    }
    let c = re(&YEAR_RE, YEAR)?.captures(text)?;
    let year = year_of(c.get(1), now)?;
    let days = if leap(year) { 366 } else { 365 };
    Some(format!("Rok {year} ma **{days} dni**."))
}

#[cfg(test)]
mod tests {
    use super::answer;
    use chrono::NaiveDate;

    fn a(text: &str) -> Option<String> {
        let now = NaiveDate::from_ymd_opt(2026, 10, 15)
            .and_then(|d| d.and_hms_opt(12, 0, 0))
            .unwrap_or_default();
        answer(text, now)
    }

    #[test]
    fn leap_years_and_month_lengths() {
        assert_eq!(
            a("czy 2028 jest rokiem przestepnym").as_deref(),
            Some("2028 **jest** rokiem przestępnym (366 dni, luty ma 29 dni).")
        );
        assert_eq!(
            a("czy rok 1900 byl przestepny").as_deref(),
            Some("1900 **nie jest** rokiem przestępnym (365 dni, luty ma 28 dni).")
        );
        assert!(
            a("czy 2000 to rok przestepny")
                .unwrap_or_default()
                .contains("**jest**")
        );
        assert_eq!(
            a("ile dni ma luty 2028").as_deref(),
            Some("Luty 2028 ma **29 dni**.")
        );
        assert_eq!(
            a("ile dni ma luty").as_deref(),
            Some("Luty 2026 ma **28 dni**.")
        );
        assert_eq!(
            a("ile dni ma ten miesiac").as_deref(),
            Some("Październik 2026 ma **31 dni**.")
        );
        assert_eq!(
            a("ile dni ma grudzien w roku 2030").as_deref(),
            Some("Grudzień 2030 ma **31 dni**.")
        );
        assert_eq!(
            a("ile dni ma rok 2024").as_deref(),
            Some("Rok 2024 ma **366 dni**.")
        );
        assert_eq!(
            a("ile dni ma ten rok").as_deref(),
            Some("Rok 2026 ma **365 dni**.")
        );
        for other in [
            "ile dni ma tydzien",
            "czy jutro bedzie padac",
            "ile dni ma wakacje",
        ] {
            assert_eq!(a(other), None, "{other}");
        }
    }
}
