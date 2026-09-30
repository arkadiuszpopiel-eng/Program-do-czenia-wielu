//! Liczebniki polskie: główne (mianownik, dopełniacz, rodzaj), porządkowe, cyfra po cyfrze.

mod ordinal;

pub use ordinal::{OrdForm, ordinal};

/// Największa liczba czytana jako liczebnik; większe → cyfra po cyfrze.
pub const MAX_CARDINAL: u64 = 999_999_999_999;

/// Rodzaj gramatyczny rzeczownika po liczebniku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    /// Męski (domyślny).
    Masc,
    /// Żeński (minuta, godzina, sekunda…).
    Fem,
    /// Nijaki (okno, zadanie…).
    Neut,
}

/// Przypadek liczebnika głównego (tylko te, których potrzebuje normalizator).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumCase {
    /// Mianownik / biernik („pięć złotych”).
    Nom,
    /// Dopełniacz („od pięciu złotych”).
    Gen,
}

/// Forma liczby mnogiej rzeczownika po liczebniku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plural {
    /// 1 → „złoty”.
    One,
    /// 2–4 (bez 12–14) → „złote”.
    Few,
    /// pozostałe → „złotych”.
    Many,
}

/// Reguła polskiej liczby mnogiej.
pub fn plural(n: u64) -> Plural {
    if n == 1 {
        return Plural::One;
    }
    let (units, tens) = (n % 10, n % 100);
    if (2..=4).contains(&units) && !(12..=14).contains(&tens) {
        Plural::Few
    } else {
        Plural::Many
    }
}

const DIGITS: [&str; 10] = [
    "zero",
    "jeden",
    "dwa",
    "trzy",
    "cztery",
    "pięć",
    "sześć",
    "siedem",
    "osiem",
    "dziewięć",
];
const TEENS: [&str; 10] = [
    "dziesięć",
    "jedenaście",
    "dwanaście",
    "trzynaście",
    "czternaście",
    "piętnaście",
    "szesnaście",
    "siedemnaście",
    "osiemnaście",
    "dziewiętnaście",
];
const TENS: [&str; 10] = [
    "",
    "",
    "dwadzieścia",
    "trzydzieści",
    "czterdzieści",
    "pięćdziesiąt",
    "sześćdziesiąt",
    "siedemdziesiąt",
    "osiemdziesiąt",
    "dziewięćdziesiąt",
];
pub(crate) const HUNDREDS: [&str; 10] = [
    "",
    "sto",
    "dwieście",
    "trzysta",
    "czterysta",
    "pięćset",
    "sześćset",
    "siedemset",
    "osiemset",
    "dziewięćset",
];
const DIGITS_GEN: [&str; 10] = [
    "zera",
    "jednego",
    "dwóch",
    "trzech",
    "czterech",
    "pięciu",
    "sześciu",
    "siedmiu",
    "ośmiu",
    "dziewięciu",
];
const TEENS_GEN: [&str; 10] = [
    "dziesięciu",
    "jedenastu",
    "dwunastu",
    "trzynastu",
    "czternastu",
    "piętnastu",
    "szesnastu",
    "siedemnastu",
    "osiemnastu",
    "dziewiętnastu",
];
const TENS_GEN: [&str; 10] = [
    "",
    "",
    "dwudziestu",
    "trzydziestu",
    "czterdziestu",
    "pięćdziesięciu",
    "sześćdziesięciu",
    "siedemdziesięciu",
    "osiemdziesięciu",
    "dziewięćdziesięciu",
];
const HUNDREDS_GEN: [&str; 10] = [
    "",
    "stu",
    "dwustu",
    "trzystu",
    "czterystu",
    "pięciuset",
    "sześciuset",
    "siedmiuset",
    "ośmiuset",
    "dziewięciuset",
];

/// Skale: (1, 2–4, 5+, dopełniacz l. poj.).
const SCALES: [(u64, [&str; 4]); 3] = [
    (
        1_000_000_000,
        ["miliard", "miliardy", "miliardów", "miliarda"],
    ),
    (1_000_000, ["milion", "miliony", "milionów", "miliona"]),
    (1_000, ["tysiąc", "tysiące", "tysięcy", "tysiąca"]),
];

pub(crate) fn idx(n: u64) -> usize {
    usize::try_from(n % 10).unwrap_or(0)
}

/// Grupa 1–999 bez rodzaju (jedynka jako „jeden”).
fn group_words(
    g: u64,
    case: NumCase,
    gender: Gender,
    is_last: bool,
    whole: u64,
    out: &mut Vec<&'static str>,
) {
    let (h, rest) = (g / 100, g % 100);
    let genitive = case == NumCase::Gen;
    if h > 0 {
        out.push(if genitive {
            HUNDREDS_GEN[idx(h)]
        } else {
            HUNDREDS[idx(h)]
        });
    }
    if (10..20).contains(&rest) {
        out.push(if genitive {
            TEENS_GEN[idx(rest)]
        } else {
            TEENS[idx(rest)]
        });
        return;
    }
    let (t, u) = (rest / 10, rest % 10);
    if t >= 2 {
        out.push(if genitive {
            TENS_GEN[idx(t)]
        } else {
            TENS[idx(t)]
        });
    }
    if u == 0 {
        return;
    }
    let word = match (u, genitive) {
        // Jedynka odmienia się tylko jako cała liczba 1 („dwadzieścia jeden” zawsze bez zmian).
        (1, _) if whole != 1 || !is_last => "jeden",
        (1, false) => match gender {
            Gender::Masc => "jeden",
            Gender::Fem => "jedna",
            Gender::Neut => "jedno",
        },
        (1, true) => match gender {
            Gender::Fem => "jednej",
            _ => "jednego",
        },
        (2, false) if is_last && gender == Gender::Fem => "dwie",
        (_, false) => DIGITS[idx(u)],
        (_, true) => DIGITS_GEN[idx(u)],
    };
    out.push(word);
}

/// Liczebnik główny 0–999 999 999 999 w podanym przypadku i rodzaju (rodzaj dotyczy
/// ostatniej grupy: „dwie minuty”, „dwa tysiące dwie minuty”). Większe → cyfra po cyfrze.
pub fn cardinal(n: u64, case: NumCase, gender: Gender) -> String {
    if n > MAX_CARDINAL {
        return digits(&n.to_string());
    }
    if n == 0 {
        return if case == NumCase::Gen { "zera" } else { "zero" }.to_owned();
    }
    let mut out: Vec<&'static str> = Vec::new();
    let mut rest = n;
    for (scale, forms) in SCALES {
        let g = rest / scale;
        rest %= scale;
        if g == 0 {
            continue;
        }
        if g == 1 {
            out.push(if case == NumCase::Gen {
                forms[3]
            } else {
                forms[0]
            });
            continue;
        }
        group_words(g, case, Gender::Masc, false, n, &mut out);
        out.push(match (case, plural(g)) {
            (NumCase::Nom, Plural::Few) => forms[1],
            _ => forms[2],
        });
    }
    if rest > 0 {
        group_words(rest, case, gender, true, n, &mut out);
    }
    out.join(" ")
}

/// Liczebnik główny w mianowniku, rodzaj męski.
pub fn cardinal_nom(n: u64) -> String {
    cardinal(n, NumCase::Nom, Gender::Masc)
}

/// Czytanie cyfra po cyfrze („007” → „zero zero siedem”); znaki niebędące cyframi są pomijane.
pub fn digits(s: &str) -> String {
    s.chars()
        .filter_map(|c| c.to_digit(10))
        .filter_map(|d| DIGITS.get(usize::try_from(d).unwrap_or(0)).copied())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Czyta ciąg cyfr: liczebnik, a przy zerach wiodących lub zbyt dużej liczbie — cyfra po cyfrze.
pub fn read_digit_string(s: &str, case: NumCase, gender: Gender) -> String {
    let leading_zero = s.len() > 1 && s.starts_with('0');
    match s.parse::<u64>() {
        Ok(n) if !leading_zero && n <= MAX_CARDINAL => cardinal(n, case, gender),
        _ => digits(s),
    }
}
