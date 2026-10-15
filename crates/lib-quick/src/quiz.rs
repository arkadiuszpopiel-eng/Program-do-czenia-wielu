//! Quiz z tabliczki mnożenia: „przepytaj mnie z tabliczki mnożenia (przez 7)”. Runda to 10 pytań;
//! odpowiedź liczbą („56”, „to 56”, „pięćdziesiąt sześć”), „koniec” przerywa z podsumowaniem.
//! Stan trzyma wywołujący (jeden quiz na rozmowę); wiadomość, która nie jest odpowiedzią,
//! kończy quiz bez słowa i idzie dalej zwykłą drogą.

use std::sync::OnceLock;

use regex::Regex;

use crate::text::{normalize, plural, re};
use crate::words;

/// Pytań w rundzie.
pub const ROUND: u8 = 10;

const START: &str = r"^(?:(?:przepytaj|odpytaj|przepytuj|sprawdz|pocwiczmy|cwiczmy|zrobmy quiz) )?(?:mnie |nas )?(?:z |quiz z )?(?:tabliczk[ia]|tabliczke) mnozenia(?: (?:przez|do|z|na) (\d{1,2}))?$";
const STOP: &str = r"^(?:koniec|dosc|starczy|przestan|stop|zakoncz|wystarczy|koniec quizu)$";
const NUMBER: &str = r"^(?:(?:to|wynik to|wynik|rowna sie|bedzie) )?(-?\d{1,4})$";

/// Stan quizu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quiz {
    table: Option<u8>,
    a: u8,
    b: u8,
    asked: u8,
    correct: u8,
}

impl Quiz {
    fn ask(&mut self, roll: &mut dyn FnMut(u64) -> u64) -> String {
        let pick = |roll: &mut dyn FnMut(u64) -> u64| u8::try_from(roll(9) + 2).unwrap_or(2);
        let (prev_a, prev_b) = (self.a, self.b);
        for _ in 0..4 {
            self.a = self.table.unwrap_or_else(|| pick(roll));
            self.b = pick(roll);
            if (self.a, self.b) != (prev_a, prev_b) {
                break;
            }
        }
        self.asked += 1;
        format!("**{} × {} = ?**", self.a, self.b)
    }

    fn summary(&self) -> String {
        let answered = if self.asked > 0 { self.asked - 1 } else { 0 };
        self.score(answered)
    }

    fn score(&self, answered: u8) -> String {
        let comment = match (self.correct, answered) {
            (_, 0) => "Wracaj, kiedy zechcesz.",
            (c, n) if c == n => "Wszystko dobrze — świetnie!",
            (c, n) if u16::from(c) * 10 >= u16::from(n) * 7 => "Bardzo dobrze.",
            (c, n) if u16::from(c) * 10 >= u16::from(n) * 4 => "Nieźle — warto jeszcze poćwiczyć.",
            _ => "Poćwiczmy jeszcze.",
        };
        format!(
            "Dobrze: {} z {} {}. {comment}",
            self.correct,
            answered,
            plural(u64::from(answered), "pytania", "pytań", "pytań")
        )
    }
}

fn spoken(text: &str) -> Option<i64> {
    let rest = ["wynik to ", "to ", "wynik ", "rowna sie ", "bedzie "]
        .iter()
        .find_map(|p| text.strip_prefix(p))
        .unwrap_or(text);
    let (minus, rest) = match rest.strip_prefix("minus ") {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let ws: Vec<&str> = rest.split(' ').collect();
    let (value, used) = words::parse(&ws)?;
    let value = i64::try_from(value).ok()?;
    (used == ws.len()).then_some(if minus { -value } else { value })
}

/// Początek quizu: stan i pierwsze pytanie.
pub fn start(message: &str, roll: &mut dyn FnMut(u64) -> u64) -> Option<(Quiz, String)> {
    static START_RE: OnceLock<Option<Regex>> = OnceLock::new();
    let text = normalize(message);
    let caps = re(&START_RE, START)?.captures(&text)?;
    let table = match caps.get(1) {
        Some(m) => Some(
            m.as_str()
                .parse::<u8>()
                .ok()
                .filter(|t| (1..=20).contains(t))?,
        ),
        None => None,
    };
    let mut quiz = Quiz {
        table,
        a: 0,
        b: 0,
        asked: 0,
        correct: 0,
    };
    let topic = match table {
        Some(t) => format!("tabliczki mnożenia przez {t}"),
        None => "tabliczki mnożenia".to_owned(),
    };
    let first = quiz.ask(roll);
    Some((
        quiz,
        format!(
            "Dobrze, popytam cię z {topic}. {ROUND} pytań — odpowiadaj liczbą, „koniec” przerywa.\n\n{first}"
        ),
    ))
}

/// Odpowiedź w trwającym quizie: tekst i czy quiz się skończył. `None` — to nie odpowiedź
/// (wiadomość idzie zwykłą drogą, quiz należy zakończyć).
pub fn reply(
    quiz: &mut Quiz,
    message: &str,
    roll: &mut dyn FnMut(u64) -> u64,
) -> Option<(String, bool)> {
    static STOP_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static NUMBER_RE: OnceLock<Option<Regex>> = OnceLock::new();
    let text = normalize(message);
    if re(&STOP_RE, STOP)?.is_match(&text) {
        return Some((format!("Kończymy. {}", quiz.summary()), true));
    }
    let given = match re(&NUMBER_RE, NUMBER)?.captures(&text) {
        Some(caps) => caps.get(1)?.as_str().parse::<i64>().ok()?,
        None => spoken(&text)?,
    };
    let right = i64::from(quiz.a) * i64::from(quiz.b);
    let verdict = if given == right {
        quiz.correct += 1;
        format!("Dobrze! {} × {} = **{right}**.", quiz.a, quiz.b)
    } else {
        format!(
            "Prawie — {} × {} = **{right}**, a nie {given}.",
            quiz.a, quiz.b
        )
    };
    if quiz.asked >= ROUND {
        return Some((
            format!("{verdict}\n\nKoniec pytań. {}", quiz.score(quiz.asked)),
            true,
        ));
    }
    let next = quiz.ask(roll);
    Some((format!("{verdict}\n\n{next}"), false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_round_with_words_and_summary() {
        let mut roll = |_n: u64| 5;
        let (mut quiz, first) =
            start("Przepytaj mnie z tabliczki mnożenia przez 7", &mut roll).unwrap();
        assert!(
            first.contains("przez 7") && first.ends_with("**7 × 7 = ?**"),
            "{first}"
        );
        let (r, done) = reply(&mut quiz, "49", &mut roll).unwrap();
        assert!(r.starts_with("Dobrze! 7 × 7 = **49**.") && !done, "{r}");
        let (r, _) = reply(&mut quiz, "to czterdzieści osiem", &mut roll).unwrap();
        assert!(r.starts_with("Prawie — 7 × 7 = **49**, a nie 48."), "{r}");
        for _ in 0..7 {
            reply(&mut quiz, "czterdzieści dziewięć", &mut roll).unwrap();
        }
        let (r, done) = reply(&mut quiz, "49", &mut roll).unwrap();
        assert!(
            done && r.contains("Koniec pytań. Dobrze: 9 z 10 pytań. Bardzo dobrze."),
            "{r}"
        );
    }

    #[test]
    fn stop_and_other_messages() {
        let mut roll = |n: u64| n - 1;
        let (mut quiz, first) = start("tabliczka mnożenia", &mut roll).unwrap();
        assert!(first.ends_with("**10 × 10 = ?**"), "{first}");
        assert_eq!(reply(&mut quiz, "Opowiedz mi bajkę", &mut roll), None);
        let (r, done) = reply(&mut quiz, "Koniec", &mut roll).unwrap();
        assert!(done && r.starts_with("Kończymy. Dobrze: 0 z 0"), "{r}");
        assert!(start("przepytaj mnie z historii", &mut roll).is_none());
        assert!(start("tabliczka mnożenia przez 99", &mut roll).is_none());
    }
}
