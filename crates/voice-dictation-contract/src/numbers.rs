//! Liczebniki główne PL (mianownik/biernik) → cyfry, **tylko gdy zapis jest jednoznaczny**:
//! złożenie z ≥ 2 słów („dwadzieścia trzy” → 23, „dwa tysiące dwadzieścia sześć” → 2026) albo
//! jedno słowo 11–99 („piętnaście” → 15, „trzydzieści” → 30). Zostają słowami: pojedyncze 0–10
//! („jeden z nich”, „dwa razy”), samotne „sto”, „tysiąc”, „milion” („sto lat”), formy odmienione
//! („pięciu”, „dwudziestu”) i porządkowe („dziewiątej”).

use personas_contract::fold;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Unit,
    Teen,
    Ten,
    Hundred,
    Thousand,
    Million,
}

fn word(w: &str) -> Option<(Kind, u64)> {
    let f = fold(w);
    let v = match f.as_str() {
        "zero" => (Kind::Unit, 0),
        "jeden" | "jedna" | "jedno" => (Kind::Unit, 1),
        "dwa" | "dwie" => (Kind::Unit, 2),
        "trzy" => (Kind::Unit, 3),
        "cztery" => (Kind::Unit, 4),
        "piec" => (Kind::Unit, 5),
        "szesc" => (Kind::Unit, 6),
        "siedem" => (Kind::Unit, 7),
        "osiem" => (Kind::Unit, 8),
        "dziewiec" => (Kind::Unit, 9),
        "dziesiec" => (Kind::Teen, 10),
        "jedenascie" => (Kind::Teen, 11),
        "dwanascie" => (Kind::Teen, 12),
        "trzynascie" => (Kind::Teen, 13),
        "czternascie" => (Kind::Teen, 14),
        "pietnascie" => (Kind::Teen, 15),
        "szesnascie" => (Kind::Teen, 16),
        "siedemnascie" => (Kind::Teen, 17),
        "osiemnascie" => (Kind::Teen, 18),
        "dziewietnascie" => (Kind::Teen, 19),
        "dwadziescia" => (Kind::Ten, 20),
        "trzydziesci" => (Kind::Ten, 30),
        "czterdziesci" => (Kind::Ten, 40),
        "piecdziesiat" => (Kind::Ten, 50),
        "szescdziesiat" => (Kind::Ten, 60),
        "siedemdziesiat" => (Kind::Ten, 70),
        "osiemdziesiat" => (Kind::Ten, 80),
        "dziewiecdziesiat" => (Kind::Ten, 90),
        "sto" => (Kind::Hundred, 100),
        "dwiescie" => (Kind::Hundred, 200),
        "trzysta" => (Kind::Hundred, 300),
        "czterysta" => (Kind::Hundred, 400),
        "piecset" => (Kind::Hundred, 500),
        "szescset" => (Kind::Hundred, 600),
        "siedemset" => (Kind::Hundred, 700),
        "osiemset" => (Kind::Hundred, 800),
        "dziewiecset" => (Kind::Hundred, 900),
        "tysiac" | "tysiace" | "tysiecy" => (Kind::Thousand, 1_000),
        "milion" | "miliony" | "milionow" => (Kind::Million, 1_000_000),
        _ => return None,
    };
    Some(v)
}

/// Czy słowo jest liczebnikiem głównym ze słownika.
pub fn is_number_word(w: &str) -> bool {
    word(w).is_some()
}

/// Najdłuższy poprawny liczebnik od początku `words`: (wartość, liczba słów).
pub fn parse_prefix(words: &[&str]) -> Option<(u64, usize)> {
    let mut total = 0u64;
    let mut group = 0u64;
    let mut last: Option<Kind> = None;
    let mut scale_seen: Option<Kind> = None;
    let mut used = 0;
    let mut best: Option<(u64, usize)> = None;
    for w in words {
        let Some((kind, v)) = word(w) else { break };
        let fits = match kind {
            Kind::Unit => matches!(
                last,
                None | Some(Kind::Hundred | Kind::Ten | Kind::Thousand | Kind::Million)
            ),
            Kind::Teen | Kind::Ten => {
                matches!(
                    last,
                    None | Some(Kind::Hundred | Kind::Thousand | Kind::Million)
                )
            }
            Kind::Hundred => matches!(last, None | Some(Kind::Thousand | Kind::Million)),
            Kind::Thousand | Kind::Million => {
                !matches!(last, Some(Kind::Thousand | Kind::Million))
                    && scale_seen.is_none_or(|s| kind < s)
            }
        };
        // „zero” tylko samodzielnie (nie „dwadzieścia zero”, nie „zero tysięcy”).
        if !fits || (v == 0 && last.is_some()) || (last == Some(Kind::Unit) && group == 0) {
            break;
        }
        match kind {
            Kind::Thousand | Kind::Million => {
                total += group.max(1) * v;
                group = 0;
                scale_seen = Some(kind);
            }
            _ => group += v,
        }
        last = Some(kind);
        used += 1;
        best = Some((total + group, used));
    }
    best
}

/// Czy liczebnik z `n` słów o wartości `v` zapisać cyframi.
pub fn unambiguous(v: u64, n: usize) -> bool {
    n >= 2 || (11..=99).contains(&v)
}
