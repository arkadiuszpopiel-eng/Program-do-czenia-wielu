//! Liczby słownie i rzymskie bez modelu: „1234 słownie”, „zapisz 1999,99 zł słownie” (jak na
//! fakturze: „… złotych 99/100”), „2026 rzymskimi”, „rzymskie MCMXC”. Liczby rzymskie tylko w postaci
//! kanonicznej (1–3999) i tylko z jawnym słowem „rzymskie” — „ile to mix” idzie do modelu.

use std::sync::OnceLock;

use regex::Regex;

use crate::text::{plural, re};

/// Największa liczba słownie (999 miliardów…).
const MAX: u64 = 999_999_999_999;

const ONES: [&str; 20] = [
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
const HUNDREDS: [&str; 10] = [
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
/// Rzędy: (jedna, kilka, wiele) — tysiące, miliony, miliardy.
const SCALES: [(&str, &str, &str); 3] = [
    ("tysiąc", "tysiące", "tysięcy"),
    ("milion", "miliony", "milionów"),
    ("miliard", "miliardy", "miliardów"),
];

fn get<'a>(table: &[&'a str], i: u64) -> &'a str {
    usize::try_from(i)
        .ok()
        .and_then(|i| table.get(i))
        .copied()
        .unwrap_or_default()
}

/// Liczba 1–999 słownie (słowa w kolejności setki, dziesiątki, jedności).
fn group(n: u64, out: &mut Vec<&'static str>) {
    let (h, rest) = (n / 100, n % 100);
    if h > 0 {
        out.push(get(&HUNDREDS, h));
    }
    if rest >= 20 {
        out.push(get(&TENS, rest / 10));
        if rest % 10 > 0 {
            out.push(get(&ONES, rest % 10));
        }
    } else if rest > 0 {
        out.push(get(&ONES, rest));
    }
}

/// Liczba słownie: „tysiąc dwieście trzydzieści cztery”, „dwa miliony pięć tysięcy”.
pub fn spelled(n: u64) -> String {
    if n == 0 {
        return "zero".to_owned();
    }
    let mut out: Vec<&'static str> = Vec::new();
    let groups = [
        (n / 1_000_000_000) % 1000,
        (n / 1_000_000) % 1000,
        (n / 1000) % 1000,
    ];
    for (g, (one, few, many)) in groups.iter().zip(SCALES.iter().rev()) {
        match *g {
            0 => {}
            1 => out.push(one),
            g => {
                group(g, &mut out);
                out.push(plural(g, one, few, many));
            }
        }
    }
    group(n % 1000, &mut out);
    out.join(" ")
}

/// Liczba rzymska (1–3999) w postaci kanonicznej.
pub fn roman(mut n: u64) -> Option<String> {
    if !(1..=3999).contains(&n) {
        return None;
    }
    const TABLE: [(u64, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (value, symbol) in TABLE {
        while n >= value {
            out.push_str(symbol);
            n -= value;
        }
    }
    Some(out)
}

/// Odczyt liczby rzymskiej; tylko postać kanoniczna (zapis zwrotny musi być identyczny).
pub fn from_roman(text: &str) -> Option<u64> {
    let upper = text.to_ascii_uppercase();
    let value = |c: char| match c {
        'I' => Some(1),
        'V' => Some(5),
        'X' => Some(10),
        'L' => Some(50),
        'C' => Some(100),
        'D' => Some(500),
        'M' => Some(1000),
        _ => None,
    };
    let digits: Vec<i64> = upper.chars().map(value).collect::<Option<_>>()?;
    let mut total: i64 = 0;
    for (i, d) in digits.iter().enumerate() {
        match digits.get(i + 1) {
            Some(next) if next > d => total -= d,
            _ => total += d,
        }
    }
    let total = u64::try_from(total).ok()?;
    // Zapis zwrotny musi być identyczny — odrzuca „IIX”, „VX”, „MMMM” i podobne.
    (roman(total)? == upper).then_some(total)
}

const SPELL: &str = r"^(?:(?:jak )?(?:zapisz|napisz|zapisac|napisac|podaj) )?(?:liczbe |kwote )?(\d{1,3}(?: \d{3})+|\d{1,12})(?:[.,](\d{1,2}))? ?(zl|zlotych|zlote|zloty|pln)? slownie$|^slownie (\d{1,3}(?: \d{3})+|\d{1,12})(?:[.,](\d{1,2}))? ?(zl|zlotych|zlote|zloty|pln)?$";
const TO_ROMAN: &str = r"^(?:(?:jak )?(?:zapisz|napisz|zapisac|napisac|zamien|przelicz) )?(?:liczbe )?(\d{1,4}) (?:na rzymskie|na rzymska|rzymskimi|po rzymsku|cyframi rzymskimi|w rzymskich)$";
const FROM_ROMAN: &str = r"^(?:(?:ile to (?:jest )?|zamien |przelicz |co to (?:jest |za liczba )?)?(?:liczba |liczbe )?rzymsk(?:ie|a|i) ([mdclxvi]{1,16})(?: na arabskie| po arabsku)?|([mdclxvi]{1,16}) (?:na arabskie|po arabsku|rzymskimi to ile))$";

fn amount(whole: &str, cents: Option<&str>, currency: bool) -> Option<String> {
    let n: u64 = whole.replace(' ', "").parse().ok()?;
    if n > MAX {
        return None;
    }
    let words = spelled(n);
    let cents = match cents {
        Some(c) if c.len() == 1 => format!("{c}0"),
        Some(c) => c.to_owned(),
        None => "00".to_owned(),
    };
    let base = crate::text::number(n as f64);
    let shown = if cents == "00" {
        base
    } else {
        format!("{base},{cents}")
    };
    Some(if currency {
        let unit = plural(n, "złoty", "złote", "złotych");
        format!("{shown} zł = **{words} {unit} {cents}/100**")
    } else if cents != "00" {
        format!("{shown} = **{words} i {cents}/100**")
    } else {
        format!("{shown} = **{words}**")
    })
}

/// Odpowiedź albo `None` (`text` po `normalize`).
pub fn answer(text: &str) -> Option<String> {
    static SPELL_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static TO_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static FROM_RE: OnceLock<Option<Regex>> = OnceLock::new();
    if let Some(c) = re(&SPELL_RE, SPELL)?.captures(text) {
        let (whole, cents, cur) = if c.get(1).is_some() {
            (c.get(1)?, c.get(2), c.get(3))
        } else {
            (c.get(4)?, c.get(5), c.get(6))
        };
        return amount(whole.as_str(), cents.map(|m| m.as_str()), cur.is_some());
    }
    if let Some(c) = re(&TO_RE, TO_ROMAN)?.captures(text) {
        let n: u64 = c.get(1)?.as_str().parse().ok()?;
        return Some(format!("{n} = **{}**", roman(n)?));
    }
    let c = re(&FROM_RE, FROM_ROMAN)?.captures(text)?;
    let raw = c.get(1).or_else(|| c.get(2))?.as_str();
    let n = from_roman(raw)?;
    Some(format!("{} = **{n}**", raw.to_ascii_uppercase()))
}

#[cfg(test)]
#[path = "numerals_tests.rs"]
mod tests;
