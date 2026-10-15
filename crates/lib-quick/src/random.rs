//! Losowanie: kostka („rzuć kostką”, „rzuć dwiema kostkami”), moneta („orzeł czy reszka”),
//! liczba („wylosuj liczbę od 1 do 10”, „podaj losową cyfrę”). Źródło losowości podaje
//! wywołujący (`roll(n)` → 0..n), testy podstawiają stałe.

use std::sync::OnceLock;

use regex::Regex;

use crate::text::re;

const DICE: &str = r"^(?:rzuc|rzut|rzucaj)(?: mi| nam)?(?: (\d|jedna|dwiema|dwoma|trzema|czterema|piecioma|szescioma))? (?:kostka|kostkami|kostke|koscia|kosc|koscmi)(?: do gry)?$";
const COIN: &str = r"^(?:(?:rzuc|rzut|rzucaj)(?: mi| nam)? (?:moneta|monete|monetka|monetke)|orzel (?:czy|albo) reszka)$";
const RANGE: &str = r"^(?:(wy)?losuj|(?:podaj|daj|wybierz)(?: mi| nam)?)(?: jakas| jedna)?(?: (losowa|przypadkowa))? ?(liczbe|numer|cyfre)?(?: (losowa|losowo|przypadkowa))?(?: (?:od|z przedzialu) (-?\d{1,12}))?(?: (?:do|-) (-?\d{1,12}))?$";

fn dice_count(word: Option<&str>) -> Option<u64> {
    Some(match word {
        None | Some("jedna") => 1,
        Some("dwiema" | "dwoma") => 2,
        Some("trzema") => 3,
        Some("czterema") => 4,
        Some("piecioma") => 5,
        Some("szescioma") => 6,
        Some(d) => d.parse().ok().filter(|n| (1..=6).contains(n))?,
    })
}

/// Odpowiedź albo `None`.
pub fn answer(text: &str, roll: &mut dyn FnMut(u64) -> u64) -> Option<String> {
    static DICE_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static COIN_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static RANGE_RE: OnceLock<Option<Regex>> = OnceLock::new();
    if let Some(caps) = re(&DICE_RE, DICE)?.captures(text) {
        let n = dice_count(caps.get(1).map(|m| m.as_str()))?;
        let rolls: Vec<u64> = (0..n).map(|_| roll(6) + 1).collect();
        if n == 1 {
            return Some(format!("Wypadło: **{}**.", rolls[0]));
        }
        let shown: Vec<String> = rolls.iter().map(|r| format!("**{r}**")).collect();
        let sum: u64 = rolls.iter().sum();
        return Some(format!("Wypadło: {} (razem {sum}).", shown.join(", ")));
    }
    if re(&COIN_RE, COIN)?.is_match(text) {
        return Some(if roll(2) == 0 {
            "Wypadł **orzeł**.".into()
        } else {
            "Wypadła **reszka**.".into()
        });
    }
    let caps = re(&RANGE_RE, RANGE)?.captures(text)?;
    let drawn = caps.get(1).is_some() || text.starts_with("losuj");
    let random = caps.get(2).is_some() || caps.get(4).is_some();
    let noun = caps.get(3).map(|m| m.as_str());
    let from = caps.get(5).and_then(|m| m.as_str().parse::<i64>().ok());
    let to = caps.get(6).and_then(|m| m.as_str().parse::<i64>().ok());
    // „podaj liczbę” bez „losową” i bez zakresu to nie losowanie.
    if !drawn && !random && to.is_none() {
        return None;
    }
    if noun.is_none() && from.is_none() && to.is_none() && !drawn {
        return None;
    }
    let digit = noun == Some("cyfre");
    let (mut lo, mut hi) = match (from, to, digit) {
        (Some(a), Some(b), _) => (a, b),
        (None, Some(b), _) => (if digit { 0 } else { 1 }, b),
        (Some(a), None, _) => (a, a.saturating_add(99)),
        (None, None, true) => (0, 9),
        (None, None, false) => (1, 100),
    };
    if lo > hi {
        std::mem::swap(&mut lo, &mut hi);
    }
    let span = u64::try_from(hi.checked_sub(lo)?).ok()?.checked_add(1)?;
    let value = lo.checked_add(i64::try_from(roll(span)).ok()?)?;
    let what = if digit { "cyfra" } else { "liczba" };
    Some(format!("Wylosowana {what} od {lo} do {hi}: **{value}**."))
}

#[cfg(test)]
mod tests {
    use super::answer;

    fn fixed(text: &str, value: u64) -> Option<String> {
        answer(text, &mut |n| value.min(n - 1))
    }

    #[test]
    fn dice_coin_and_ranges() {
        assert_eq!(fixed("rzuc kostka", 3).as_deref(), Some("Wypadło: **4**."));
        assert_eq!(
            fixed("rzuc dwiema kostkami", 5).as_deref(),
            Some("Wypadło: **6**, **6** (razem 12).")
        );
        assert_eq!(
            fixed("orzel czy reszka", 0).as_deref(),
            Some("Wypadł **orzeł**.")
        );
        assert_eq!(
            fixed("rzuc moneta", 1).as_deref(),
            Some("Wypadła **reszka**.")
        );
        assert_eq!(
            fixed("wylosuj liczbe od 1 do 10", 0).as_deref(),
            Some("Wylosowana liczba od 1 do 10: **1**.")
        );
        assert_eq!(
            fixed("losuj od 10 do 1", 100).as_deref(),
            Some("Wylosowana liczba od 1 do 10: **10**.")
        );
        assert_eq!(
            fixed("podaj losowa cyfre", 7).as_deref(),
            Some("Wylosowana cyfra od 0 do 9: **7**.")
        );
        assert_eq!(
            fixed("losuj", 41).as_deref(),
            Some("Wylosowana liczba od 1 do 100: **42**.")
        );
        assert_eq!(
            fixed("podaj liczbe", 1),
            None,
            "bez „losową” to nie losowanie"
        );
        assert_eq!(fixed("daj mi numer", 1), None);
        assert_eq!(fixed("rzuc kostka i powiedz", 1), None);
    }
}
