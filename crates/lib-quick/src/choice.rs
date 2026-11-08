//! Losowy wybór z listy podanej w wiadomości po dwukropku: „wybierz jedno: pizza, sushi, żurek”,
//! „wylosuj coś z: kino albo spacer” („Wybierz coś na obiad, jestem głodny” idzie do modelu).
//! Pozycje w oryginalnej pisowni (z polskimi znakami); 2–20 pozycji. Źródło losowości podaje
//! wywołujący (`roll(n)` → 0..n).

use std::sync::OnceLock;

use regex::Regex;

use crate::text::{fold, re};

const CHOICE: &str = r"^(?:(?:hej|czesc)[ ,]+)?(?:alfa[ ,:]+)?(?:prosze[ ,]+)?(?:wybierz|wylosuj|losuj)(?: mi| nam| za mnie)? (?:jedno|jedna|jeden|cos|cokolwiek|ktores|ktoras|ktorys)(?: (?:z|sposrod)(?: (?:tych|ponizszych|listy|dwoch|trzech|czterech|pieciu))?)?: (.+)$";
const SEPARATOR: &str = r"(?i)\s*(?:[,;/]|\s(?:albo|czy|lub)\s)\s*";
const MAX_ITEMS: usize = 20;
const MAX_ITEM: usize = 60;

/// Odpowiedź albo `None` (`message` — oryginalna wiadomość).
pub fn answer(message: &str, roll: &mut dyn FnMut(u64) -> u64) -> Option<String> {
    static CHOICE_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static SEP_RE: OnceLock<Option<Regex>> = OnceLock::new();
    let joined = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = joined.trim_end_matches(['?', '!', '.', '…', ' ']);
    // `fold` zamienia znak na znak — pozycje w tekście bez polskich znaków odpowiadają oryginałowi.
    let folded = fold(trimmed);
    if folded.chars().count() != trimmed.chars().count() {
        return None;
    }
    let caps = re(&CHOICE_RE, CHOICE)?.captures(&folded)?;
    let list = caps.get(1)?;
    let start = folded.get(..list.start())?.chars().count();
    let len = list.as_str().chars().count();
    let original: String = trimmed.chars().skip(start).take(len).collect();
    let items: Vec<&str> = re(&SEP_RE, SEPARATOR)?
        .split(&original)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if items.len() < 2
        || items.len() > MAX_ITEMS
        || items.iter().any(|s| s.chars().count() > MAX_ITEM)
    {
        return None;
    }
    let n = u64::try_from(items.len()).ok()?;
    let pick = usize::try_from(roll(n)).ok()?;
    let chosen = items.get(pick)?;
    Some(format!("Wybieram: **{chosen}**."))
}

#[cfg(test)]
mod tests {
    use super::answer;

    #[test]
    fn picks_from_the_list_in_original_spelling() {
        assert_eq!(
            answer("Wybierz jedno: pizza, sushi, żurek", &mut |n| n - 1).as_deref(),
            Some("Wybieram: **żurek**.")
        );
        assert_eq!(
            answer("Alfa, wylosuj coś z: kino albo spacer?", &mut |_| 0).as_deref(),
            Some("Wybieram: **kino**.")
        );
        assert_eq!(
            answer("losuj jedno spośród: Ala; Ola; Łucja", &mut |_| 2).as_deref(),
            Some("Wybieram: **Łucja**.")
        );
        assert_eq!(
            answer("wybierz jedno z dwóch: kino czy teatr", &mut |_| 1).as_deref(),
            Some("Wybieram: **teatr**.")
        );
        for other in [
            "Wybierz coś na obiad, jestem głodny",
            "Wybierz jedno słowo, które rymuje się z kot",
            "Wylosuj coś ciekawego, np. film albo książkę",
            "wybierz jedno: pizza",
            "wybierz najlepszy laptop do 3000 zł",
            "wybierz jedno: , ,",
        ] {
            assert_eq!(answer(other, &mut |_| 0), None, "{other}");
        }
    }
}
