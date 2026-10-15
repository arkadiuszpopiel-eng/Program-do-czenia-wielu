//! Literowanie: „przeliteruj rower”, „jak się literuje słowo żółw”, „jak się pisze wyraz
//! źdźbło”. Jedno słowo (do 40 liter); litery z oryginału — z polskimi znakami i wielkimi literami.

use std::sync::OnceLock;

use regex::Regex;

use crate::text::{fold, plural, re};

const PATTERN: &str = r"^(?:(?:prze|z)?literuj(?: mi| nam)?(?: slowo| wyraz)?|jak (?:sie )?literuje(?: sie)?(?: slowo| wyraz)?|jak (?:sie )?pisze (?:slowo|wyraz)) ([\p{L}'-]{1,40})$";

/// Słowo z oryginalnej wiadomości (ten sam wyraz po `fold`, bez cudzysłowów i interpunkcji).
fn original(message: &str, folded: &str) -> Option<String> {
    message
        .split_whitespace()
        .rev()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()))
        .find(|w| fold(w) == folded)
        .map(str::to_owned)
}

/// Odpowiedź albo `None`.
pub fn answer(message: &str, text: &str) -> Option<String> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    let cleaned = text.replace(['„', '”', '“', '"', '«', '»'], "");
    let caps = re(&RE, PATTERN)?.captures(cleaned.trim())?;
    let target = caps.get(1)?.as_str().trim_matches(['\'', '-']);
    let word = original(message, target).unwrap_or_else(|| target.to_owned());
    let letters: Vec<String> = word
        .chars()
        .filter(|c| c.is_alphabetic())
        .map(|c| c.to_uppercase().collect())
        .collect();
    if letters.is_empty() {
        return None;
    }
    let n = letters.len() as u64;
    Some(format!(
        "**{word}**: {} ({n} {}).",
        letters.join(" – "),
        plural(n, "litera", "litery", "liter")
    ))
}

#[cfg(test)]
mod tests {
    use super::answer;
    use crate::text::normalize;

    fn a(message: &str) -> Option<String> {
        answer(message, &normalize(message))
    }

    #[test]
    fn spells_with_polish_letters() {
        assert_eq!(
            a("Przeliteruj rower").as_deref(),
            Some("**rower**: R – O – W – E – R (5 liter).")
        );
        assert_eq!(
            a("Jak się literuje słowo „żółw”?").as_deref(),
            Some("**żółw**: Ż – Ó – Ł – W (4 litery).")
        );
        assert!(
            a("jak się pisze wyraz Źdźbło")
                .unwrap_or_default()
                .contains("Ź – D – Ź – B – Ł – O")
        );
        assert_eq!(a("jak się pisze podanie"), None, "pytanie o pisanie pisma");
        assert_eq!(a("przeliteruj Jan Kowalski"), None, "jedno słowo");
    }
}
