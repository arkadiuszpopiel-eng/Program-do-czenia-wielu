//! Data i godzina z zegara tego komputera: „która godzina”, „jaki dziś dzień”, „jaka jest
//! data”. Pytania o inne miejsce („która godzina w Tokio”) idą do modelu.

use std::sync::OnceLock;

use chrono::{Datelike, NaiveDateTime, Timelike};
use regex::Regex;

use crate::text::re;

const TIME: &str = r"^(?:(?:ktora|jaka) (?:jest |mamy )?(?:teraz )?godzina(?: jest| teraz)?|ktora (?:jest|mamy)(?: teraz)?|podaj (?:aktualna |obecna )?godzine)$";
const DATE: &str = r"^(?:(?:jaki|ktory) (?:dzis |dzisiaj |teraz )?(?:jest |mamy )?(?:dzis |dzisiaj )?dzien(?: tygodnia)?(?: jest| mamy)?(?: dzis| dzisiaj)?|(?:jaka|ktora) (?:jest )?(?:dzis |dzisiaj )?data(?: dzis| dzisiaj)?|dzisiejsza data|podaj (?:dzisiejsza |aktualna )?date|ktorego (?:jest |mamy )?(?:dzis|dzisiaj)|ktory (?:jest |mamy )?(?:dzis|dzisiaj))$";

pub(crate) const DAYS: [&str; 7] = [
    "poniedziałek",
    "wtorek",
    "środa",
    "czwartek",
    "piątek",
    "sobota",
    "niedziela",
];
pub(crate) const MONTHS: [&str; 12] = [
    "stycznia",
    "lutego",
    "marca",
    "kwietnia",
    "maja",
    "czerwca",
    "lipca",
    "sierpnia",
    "września",
    "października",
    "listopada",
    "grudnia",
];

/// Odpowiedź albo `None`.
pub fn answer(text: &str, now: NaiveDateTime) -> Option<String> {
    static TIME_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static DATE_RE: OnceLock<Option<Regex>> = OnceLock::new();
    if re(&TIME_RE, TIME)?.is_match(text) {
        return Some(format!("Jest godzina {}:{:02}.", now.hour(), now.minute()));
    }
    if re(&DATE_RE, DATE)?.is_match(text) {
        let day = DAYS.get(now.weekday().num_days_from_monday() as usize)?;
        let month = MONTHS.get(now.month0() as usize)?;
        return Some(format!(
            "Dziś jest {day}, {} {month} {}.",
            now.day(),
            now.year()
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::answer;
    use chrono::NaiveDate;

    #[test]
    fn time_and_date() {
        let now = NaiveDate::from_ymd_opt(2026, 10, 15)
            .and_then(|d| d.and_hms_opt(9, 5, 0))
            .unwrap_or_default();
        assert_eq!(
            answer("ktora godzina", now).as_deref(),
            Some("Jest godzina 9:05.")
        );
        assert_eq!(
            answer("ktora jest", now).as_deref(),
            Some("Jest godzina 9:05.")
        );
        let date = Some("Dziś jest czwartek, 15 października 2026.");
        for q in [
            "jaki dzis dzien",
            "jaki mamy dzisiaj dzien",
            "jaka jest data",
            "ktorego dzis",
        ] {
            assert_eq!(answer(q, now).as_deref(), date, "{q}");
        }
        assert_eq!(answer("ktora godzina w tokio", now), None);
        assert_eq!(answer("jaki dzien byl wczoraj", now), None);
    }
}
