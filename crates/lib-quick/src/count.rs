//! Liczenie znaków i słów w podanym tekście bez modelu: „policz znaki: Ala ma kota”, „ile słów ma:
//! …”, „ile znaków ma tekst «…»” — długość SMS-a, wpisu albo tytułu. Tekst w oryginalnej pisowni.

use std::sync::OnceLock;

use regex::Regex;

use crate::text::{fold, plural, re};

/// Początek polecenia; sam tekst musi stać po dwukropku albo w cudzysłowie — „Ile liter ma alfabet
/// polski?” to pytanie do modelu, nie tekst do policzenia.
const COUNT: &str = r"^(?:policz|zlicz|ile)(?: mi| jest| ma)? (?:znaki i slowa|slowa i znaki|znakow|znaki|slowa|slow|litery|liter)(?: (?:w tekscie|w zdaniu|ma tekst|ma zdanie|w|ma|jest))?";
const OPEN: [char; 5] = ['„', '"', '«', '“', '\''];
const CLOSE: [char; 5] = ['”', '"', '»', '“', '\''];

/// Tekst do policzenia z reszty wiadomości: po dwukropku (dokładnie, z odstępami) albo między
/// pierwszym otwierającym a ostatnim zamykającym cudzysłowem (dalej najwyżej znaki zapytania itp.).
fn subject(rest: &str) -> Option<&str> {
    let rest = rest.trim_start();
    if let Some(text) = rest.strip_prefix(':') {
        return Some(text.trim());
    }
    let inner = rest.strip_prefix(OPEN)?;
    let close = inner.rfind(CLOSE)?;
    let tail = inner.get(close..)?.chars().skip(1);
    if tail.clone().any(|c| c.is_alphanumeric()) {
        return None;
    }
    inner.get(..close)
}

/// Odpowiedź albo `None` (`message` — oryginalna wiadomość).
pub fn answer(message: &str) -> Option<String> {
    static COUNT_RE: OnceLock<Option<Regex>> = OnceLock::new();
    let original = message.trim();
    // `fold` zamienia znak na znak — pozycje w tekście bez polskich znaków odpowiadają oryginałowi.
    let folded = fold(original);
    if folded.chars().count() != original.chars().count() {
        return None;
    }
    let found = re(&COUNT_RE, COUNT)?.find(&folded)?;
    let start = folded.get(..found.end())?.chars().count();
    let at = original
        .char_indices()
        .nth(start)
        .map_or(original.len(), |(i, _)| i);
    let text = subject(original.get(at..)?)?;
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
        assert_eq!(
            answer("ile znaków ma „Ala ma kota”?").as_deref(),
            Some("**11** znaków (9 bez spacji, 9 liter), **3** słowa.")
        );
        assert_eq!(
            answer("policz znaki w tekście: Ala  ma").as_deref(),
            Some("**7** znaków (5 bez spacji, 5 liter), **2** słowa.")
        );
        for other in [
            "policz znaki:",
            "policz mi to",
            "Ile liter ma alfabet polski?",
            "Ile znaków ma numer PESEL?",
            "Ile słów ma język polski?",
            "Ile liter ma słowo kot",
            "ile słów ma „Pan Tadeusz” Mickiewicza",
        ] {
            assert_eq!(answer(other), None, "{other}");
        }
    }
}
