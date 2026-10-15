//! Liczenie znaków i słów w podanym tekście bez modelu: „policz znaki: Ala ma kota”, „ile słów ma:
//! …”, „ile znaków ma tekst «…»” — długość SMS-a, wpisu albo tytułu. Tekst w oryginalnej pisowni.

use std::sync::OnceLock;

use regex::Regex;

use crate::text::{fold, plural, re};

const COUNT: &str = r"^(?:policz|zlicz|ile)(?: mi| jest| ma)? (?:znaki i slowa|slowa i znaki|znaki|znakow|slowa|slow|litery|liter)(?: (?:ma|jest|w|w tekscie|w zdaniu|tekst|zdanie))?:? (.+)$";

/// Odpowiedź albo `None` (`message` — oryginalna wiadomość).
pub fn answer(message: &str) -> Option<String> {
    static COUNT_RE: OnceLock<Option<Regex>> = OnceLock::new();
    let joined = message.split_whitespace().collect::<Vec<_>>().join(" ");
    // `fold` zamienia znak na znak — pozycje w tekście bez polskich znaków odpowiadają oryginałowi.
    let folded = fold(&joined);
    if folded.chars().count() != joined.chars().count() {
        return None;
    }
    let caps = re(&COUNT_RE, COUNT)?.captures(&folded)?;
    let list = caps.get(1)?;
    let start = folded.get(..list.start())?.chars().count();
    let text: String = joined.chars().skip(start).collect();
    let text = text
        .trim()
        .trim_matches(['"', '„', '”', '«', '»', '\'', '“'])
        .trim();
    if text.is_empty() {
        return None;
    }
    let chars = text.chars().count();
    let no_spaces = text.chars().filter(|c| !c.is_whitespace()).count();
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    let words = text
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '\'')
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count();
    let n = |k: usize| u64::try_from(k).unwrap_or(u64::MAX);
    Some(format!(
        "**{chars}** {} ({no_spaces} bez spacji, {letters} {}), **{words}** {}.",
        plural(n(chars), "znak", "znaki", "znaków"),
        plural(n(letters), "litera", "litery", "liter"),
        plural(n(words), "słowo", "słowa", "słów"),
    ))
}

#[cfg(test)]
mod tests {
    use super::answer;

    #[test]
    fn counts_characters_and_words() {
        assert_eq!(
            answer("Policz znaki: Ala ma kota").as_deref(),
            Some("**11** znaków (9 bez spacji, 9 liter), **3** słowa.")
        );
        assert_eq!(
            answer("ile słów ma „Zażółć gęślą jaźń”").as_deref(),
            Some("**17** znaków (15 bez spacji, 15 liter), **3** słowa.")
        );
        assert_eq!(
            answer("policz słowa: biało-czerwona flaga, 2 sztuki").as_deref(),
            Some("**30** znaków (27 bez spacji, 24 litery), **4** słowa.")
        );
        assert_eq!(answer("policz znaki:"), None);
        assert_eq!(answer("policz mi to"), None);
    }
}
